use rectangle_device_blocks::{
    package::Package, raw::RawBlockFile, BlockInfo, BlockUsage, BLOCK_MAX_BYTES,
};
use rectangle_device_sandbox::{ffmpeg, socket::SocketPool};

use crate::{
    hls::{SEGMENT_MAX_SEC, SEGMENT_MIN_SEC},
    MediaBlockInfo, MediaContainer, MediaUpdate, MediaUpdateBus,
};

use async_std::{io::ReadExt, os::unix::net::UnixStream, stream::StreamExt, sync::Sender, task};
use mpeg2ts::{
    time::ClockReference,
    ts::{ReadTsPacket, TsPacket, TsPacketReader, TsPacketWriter, WriteTsPacket},
};
use std::{
    error::Error,
    io::Cursor,
    process::ExitStatus,
    thread,
    thread::JoinHandle,
    time::{Duration, Instant},
};
use thiserror::Error;

pub struct VideoIngest {
    media_sender: Sender<MediaUpdate>,
    mc: MediaContainer,
    publish_interval: Duration,
    next_publish_at: Instant,

    clock_latest: Option<ClockReference>,
    clock_first: Option<ClockReference>,
    segment_clock: Option<ClockReference>,
    program_association_table: Option<TsPacket>,
}

#[derive(Error, Debug)]
pub enum VideoIngestError {
    #[error("ffmpeg container returned unsuccessful status code `{0}`")]
    ProcessFailed(ExitStatus),
}

impl VideoIngest {
    pub fn new(mub: &MediaUpdateBus, publish_interval: Duration) -> VideoIngest {
        let next_publish_at = Instant::now() + publish_interval;
        VideoIngest {
            next_publish_at,
            publish_interval,
            mc: MediaContainer::new(),
            media_sender: mub.sender.clone(),
            clock_latest: None,
            clock_first: None,
            segment_clock: None,
            program_association_table: None,
        }
    }

    pub fn run(self, args: Vec<String>) -> Result<JoinHandle<MediaContainer>, Box<dyn Error>> {
        let thread = thread::Builder::new().name("ingest".to_string());
        Ok(thread.spawn(move || self.thread_main(args))?)
    }

    fn thread_main(self, args: Vec<String>) -> MediaContainer {
        let tc = ffmpeg::TranscodeConfig {
            image: ffmpeg::default_image(),
            args,
            allow_networking: true,
            segment_time: SEGMENT_MIN_SEC,
        };
        let mc = task::block_on(self.task(tc)).unwrap();
        log::trace!("finished successfully");
        mc
    }

    async fn push_video_block(&mut self, data: &[u8], duration: f32) {
        log::debug!("block is {} bytes, {} seconds", data.len(), duration);

        // This is where the hash computation happens
        let file = RawBlockFile::new(data);

        let info = MediaBlockInfo {
            cid: file.block.cid.clone(),
            bytes: data.len(),
            duration,
            sequence: self.mc.blocks.len(),
        };

        let sequence = info.sequence;
        let usage = BlockUsage::VideoSegment(sequence);
        self.mc.blocks.push(info);

        for block in file.into_blocks() {
            log::trace!("sending {} {:?}", block.cid.to_string(), usage);
            self.media_sender
                .send(MediaUpdate::Block(BlockInfo {
                    usage: usage.clone(),
                    block,
                }))
                .await;
        }
    }

    async fn send_container(&self) {
        self.media_sender
            .send(MediaUpdate::Container(self.mc.clone()))
            .await;
    }

    async fn send_container_periodically(&mut self) {
        let now = Instant::now();
        if now > self.next_publish_at {
            self.next_publish_at = now + self.publish_interval;
            self.send_container().await;
        }
    }

    fn inspect_packet(&mut self, packet: &TsPacket) {
        // Save a copy of the PAT (Program Association Table) so we can reinsert if necessary
        if packet.header.pid.as_u16() == mpeg2ts::ts::Pid::PAT {
            self.program_association_table = Some(packet.clone());
        }

        // Save timestamp
        if let Some(pcr) = packet.adaptation_field.as_ref().and_then(|a| a.pcr) {
            self.clock_latest = Some(pcr);
            if self.clock_first.is_none() {
                self.clock_first = self.clock_latest;
            }
        }
    }

    fn clock_latest_as_u64(&self) -> u64 {
        self.clock_latest
            .or(self.clock_first)
            .map_or(0, |c| c.as_u64())
    }

    fn segment_clock_as_u64(&self) -> u64 {
        self.segment_clock
            .or(self.clock_first)
            .map_or(0, |c| c.as_u64())
    }

    fn latest_segment_duration(&self) -> f32 {
        let segment_ticks = self.clock_latest_as_u64() - self.segment_clock_as_u64();
        (segment_ticks as f32) * (1.0 / (ClockReference::RESOLUTION as f32))
    }

    async fn task(mut self, tc: ffmpeg::TranscodeConfig) -> Result<MediaContainer, Box<dyn Error>> {
        log::info!("ingest process starting, {:?}", tc);

        // Use one socket for the segment output; ffmpeg will re-open it for each segment
        let mut pool = SocketPool::new()?;
        let output = pool.bind("/out/0.ts").await?;
        let mut incoming = output.listener.incoming();

        let mut child = ffmpeg::start(tc, &pool).await?;

        let mut read_buffer = Vec::with_capacity(BLOCK_MAX_BYTES);
        let mut write_buffer = Vec::with_capacity(BLOCK_MAX_BYTES);

        // Asynchronously set up a way to break out of the incoming connection
        // loop when the child process exits, by sending a zero length segment
        let socket_path = output.socket_path.clone();
        let status_notifier = task::spawn(async move {
            let status = child.status().await;
            UnixStream::connect(socket_path).await.unwrap();
            status
        });

        while let Some(Ok(mut stream)) = incoming.next().await {
            // Beginning of a new segment from ffmpeg
            read_buffer.clear();
            write_buffer.clear();
            self.segment_clock = self.clock_latest;

            // Get the entire segment for now, since TsPacketReader seems like it
            // will read a partial packet and lose data. To do: get a better parser?
            stream.read_to_end(&mut read_buffer).await?;
            if read_buffer.is_empty() {
                break;
            }
            log::debug!("segment is {} bytes", read_buffer.len());

            let mut ts_reader = TsPacketReader::new(Cursor::new(&mut read_buffer));
            let mut write_cursor = Cursor::new(&mut write_buffer);

            while let Ok(Some(packet)) = ts_reader.read_ts_packet() {
                let write_position = write_cursor.position() as usize;
                let duration_before_packet = self.latest_segment_duration();
                self.inspect_packet(&packet);

                // Split this segment into smaller blocks when we need to
                if write_position + TsPacket::SIZE > BLOCK_MAX_BYTES
                    || self.latest_segment_duration() >= SEGMENT_MAX_SEC
                {
                    // Generate a block which ends just before 'packet'
                    self.push_video_block(
                        &write_cursor.get_ref()[0..write_position],
                        duration_before_packet,
                    )
                    .await;

                    write_cursor.set_position(0);
                    write_cursor.get_mut().clear();

                    if let Some(pat) = &self.program_association_table {
                        TsPacketWriter::new(&mut write_cursor).write_ts_packet(pat)?;
                    }
                    self.segment_clock = self.clock_latest;
                }

                TsPacketWriter::new(&mut write_cursor).write_ts_packet(&packet)?;
            }

            self.push_video_block(write_cursor.into_inner(), self.latest_segment_duration())
                .await;

            self.send_container_periodically().await;
        }

        let status = status_notifier.await?;
        log::trace!("{:?}", status);

        if status.success() {
            log::warn!("ingest stream ended successfully, final container");
            self.send_container().await;
            Ok(self.mc)
        } else {
            Err(Box::new(VideoIngestError::ProcessFailed(status)))
        }
    }
}
