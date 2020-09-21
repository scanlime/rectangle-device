// This code may not be used for any purpose. Be gay, do crime.

use async_std::stream::StreamExt;
use async_std::sync::{Sender, channel};
use async_std::os::unix::net::UnixListener;
use async_std::io::BufReader;
use async_std::task;
use async_std::io::ReadExt;
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use mpeg2ts::time::ClockReference;
use std::error::Error;
use std::io::Cursor;
use std::process::{Command, Stdio, Child, ExitStatus};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use tempfile::TempDir;
use thiserror::Error;
use libp2p::PeerId;
use libipld::Cid;
use crate::config;
use crate::blocks::{BlockUsage, BlockInfo, RawFileBlock};
use crate::media::{MediaBlockInfo, MediaContainer};
use crate::media::hls::HLSContainer;
use crate::media::html::{HLSPlayer, HLSPlayerDist};
use crate::sandbox::ffmpeg;
use crate::sandbox::socket::SocketPool;

pub struct VideoIngest {
    block_sender: Sender<BlockInfo>,
    pin_sender: Sender<Cid>,
    hls_dist: HLSPlayerDist,
    local_peer_id: PeerId,
    mc: MediaContainer,
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
    pub fn new(block_sender: Sender<BlockInfo>, pin_sender: Sender<Cid>, local_peer_id: PeerId) -> VideoIngest {
        let hls_dist = HLSPlayerDist::new();
        VideoIngest {
            block_sender,
            pin_sender,
            hls_dist,
            local_peer_id,
            mc: MediaContainer { blocks: vec![] },
            clock_latest: None,
            clock_first: None,
            segment_clock: None,
            program_association_table: None,
        }
    }

    pub fn run(self, args: Vec<String>) -> Result<MediaContainer, Box<dyn Error>> {
        let tc = ffmpeg::TranscodeConfig {
            image: ffmpeg::default_image(),
            args,
            allow_networking: true,
            segment_time: config::SEGMENT_MIN_SEC,
        };
        task::block_on(self.task(tc))
    }

    async fn send_player(&self) {
        let hls = HLSContainer::new(&self.mc);
        let player = HLSPlayer::from_hls(&hls, &self.hls_dist, &self.local_peer_id);
        let player_cid = player.directory.block.cid.clone();

        log::info!("PLAYER created ====> https://{}.ipfs.{} ({} bytes)",
            player_cid.to_string(),
            config::IPFS_GATEWAY,
            player.directory.total_size);

        hls.send(&self.block_sender).await;
        player.send(&self.block_sender).await;
        self.pin_sender.send(player_cid).await;
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

    async fn task(mut self, tc: ffmpeg::TranscodeConfig) -> Result<MediaContainer, Box<dyn Error>> {
        log::info!("ingest process starting, {:?}", tc);

        // This is a good time to make sure the receiver has our player dependencies too
        self.hls_dist.clone().send(&self.block_sender).await;

        // Use one socket for the segment output; ffmpeg will re-open it for each segment
        let mut pool = SocketPool::new()?;
        let output = pool.bind("/out/0.ts").await?;
        let mut incoming = output.incoming();

        let mut child = ffmpeg::start(tc, &pool)?;

        let mut buffer = Vec::with_capacity(config::SEGMENT_MAX_BYTES);
        let mut next_publish_at = Instant::now() + Duration::from_secs(config::PUBLISH_INTERVAL_SEC);

        while let Some(Ok(mut stream)) = incoming.next().await {
            assert!(buffer.is_empty());
            stream.read_to_end(&mut buffer).await;
            log::info!("segment is {} bytes", buffer.len());

            let mut ts_reader = TsPacketReader::new(Cursor::new(&mut buffer));
            while let Ok(Some(packet)) = ts_reader.read_ts_packet() {
                self.inspect_packet(&packet);
            }

            let now = Instant::now();
            if now > next_publish_at {
                next_publish_at = now + Duration::from_secs(config::PUBLISH_INTERVAL_SEC);
                task::block_on(self.send_player());
            }
        }

        let status = child.wait()?;
        if status.success() {
            log::warn!("ingest stream ended successfully, final player");
            task::block_on(async { self.send_player().await });
            Ok(self.mc)
        } else {
            Err(Box::new(VideoIngestError::ProcessFailed(status)))
        }
    }
}


/*

            // What would the segment size be if we output one right before 'packet'
            let segment_bytes = cursor.position() as usize;
            let segment_ticks =
                clock_latest.or(clock_first).map_or(0, |c| c.as_u64()) -
                segment_clock.or(clock_first).map_or(0, |c| c.as_u64());
            let segment_sec = (segment_ticks as f32) * (1.0 / (ClockReference::RESOLUTION as f32));

            // To do: determining keyframes properly actually.
            // current strategy uses ffmpeg's segmentation as much as possible.
            let is_keyframe = edge_flag;

            // Split on keyframes, but respect our hard limits on time and size
            if (is_keyframe && segment_bytes >= config::SEGMENT_MIN_BYTES && segment_sec >= config::SEGMENT_MIN_SEC) ||
               (segment_bytes + TsPacket::SIZE > config::SEGMENT_MAX_BYTES) ||
               (segment_sec >= config::SEGMENT_MAX_SEC) {

                cursor.set_position(0);
                let segment = cursor.get_ref().get(0..segment_bytes).unwrap();

                // Hash the video segment here, then send it to the other task for storage
                let segment_file = RawFileBlock::new(segment);
                let segment = MediaBlockInfo {
                    cid: segment_file.block.cid.clone(),
                    bytes: segment_bytes,
                    duration: segment_sec,
                    sequence: container.blocks.len()
                };
                task::block_on(async {
                    log::trace!("video segment {}", segment.sequence);
                    segment_file.send(&self.block_sender, BlockUsage::VideoSegment(segment.sequence)).await
                });

                // Add each block to a table of contents, which is sent less frequently
                container.blocks.push(segment);


                // Each segment starts with a PAT so the other packets can be identified
                if let Some(pat) = &program_association_table {
                    TsPacketWriter::new(&mut cursor).write_ts_packet(pat).unwrap();
                }

                // This 'packet' will be the first in a new segment after the PAT
                segment_clock = clock_latest;
            }

            TsPacketWriter::new(&mut cursor).write_ts_packet(&packet).unwrap();
        }
        */
