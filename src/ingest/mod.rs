// This code may not be used for any purpose. Be gay, do crime.

use async_std::stream::StreamExt;
use async_std::sync::{Sender, channel};
use async_std::os::unix::net::UnixListener;
use async_std::task;
use async_std::io::ReadExt;
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use mpeg2ts::time::ClockReference;
use std::error::Error;
use std::io::Cursor;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use tempfile::TempDir;
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
}

impl VideoIngest {
    pub fn new(block_sender: Sender<BlockInfo>, pin_sender: Sender<Cid>, local_peer_id: PeerId) -> VideoIngest {
        let hls_dist = HLSPlayerDist::new();
        VideoIngest { block_sender, pin_sender, hls_dist, local_peer_id }
    }

    pub fn run(self, args: Vec<String>) {
        log::info!("ingest process starting, {:?}", args);

        let tc = ffmpeg::TranscodeConfig {
            image: ffmpeg::default_image(),
            args,
            allow_networking: true,
            segment_time: config::SEGMENT_MIN_SEC,
        };

        let (pool, output) = task::block_on(async {
            let mut pool = SocketPool::new().unwrap();
            let output = pool.bind("/out/0.ts").await.unwrap();
            (pool, output)
        });

        let child = ffmpeg::start(tc, &pool).unwrap();

        task::block_on(async {
            let mut incoming = output.incoming();

            while let Some(Ok(stream)) = incoming.next().await {
                println!("new stream");
                let mut stream = stream;

                let mut buf = vec![0 as u8; 1024*1024];
                while let Ok(size) = stream.read(&mut buf).await {
                    println!("got {} bytes", size);
                    if size == 0 {
                        break;
                    }
                }

                println!("end stream");
            }
        });

        let mut segment_buffer = [0 as u8; config::SEGMENT_MAX_BYTES];
        let mut cursor = Cursor::new(&mut segment_buffer[..]);
        let mut container = MediaContainer { blocks: vec![] };
        let mut next_publish_at = Instant::now() + Duration::from_secs(config::PUBLISH_INTERVAL_SEC);

        let mut clock_latest: Option<ClockReference> = None;
        let mut clock_first: Option<ClockReference> = None;
        let mut segment_clock: Option<ClockReference> = None;
        let mut program_association_table: Option<TsPacket> = None;

        // Make sure the receiver has a copy of our dependency bundle.
        task::block_on(async {
            self.hls_dist.clone().send(&self.block_sender).await;
        });


/*

fn ts_packet_pump(id: usize, listener: UnixListener, sender: Sender<(usize, TsPacket)>) {
    for stream in listener.incoming() {
        let mut reader = TsPacketReader::new(stream.unwrap());
        while let Some(packet) = reader.read_ts_packet().unwrap() {
            task::block_on(sender.send((id, packet)));
        };
    }
}

        while let Some(packet) = Some(TsPacket::new()) {

            // Save a copy of the PAT (Program Association Table) and reinsert it at every segment
            if packet.header.pid.as_u16() == mpeg2ts::ts::Pid::PAT {
                program_association_table = Some(packet.clone());
            }

            // What would the segment size be if we output one right before 'packet'
            let segment_bytes = cursor.position() as usize;
            let segment_ticks =
                clock_latest.or(clock_first).map_or(0, |c| c.as_u64()) -
                segment_clock.or(clock_first).map_or(0, |c| c.as_u64());
            let segment_sec = (segment_ticks as f32) * (1.0 / (ClockReference::RESOLUTION as f32));

            // To do: determining keyframes properly actually.
            // current strategy uses ffmpeg's segmentation as much as possible.
            let is_keyframe = edge_flag;

            // This is the most recent timestamp we know about as of 'packet'
            if let Some(pcr) = packet.adaptation_field.as_ref().and_then(|a| a.pcr) {
                clock_latest = Some(pcr);
                if clock_first.is_none() {
                    clock_first = clock_latest;
                }
            }

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

                let now = Instant::now();
                if now > next_publish_at {
                    next_publish_at = now + Duration::from_secs(config::PUBLISH_INTERVAL_SEC);
                    task::block_on(self.send_player(&container));
                }

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
        loop {
            task::block_on(async {
                log::warn!("ingest stream ended, final player");
                self.send_player(&container).await;
                task::sleep(Duration::from_secs(60)).await;
            });
        }
    }

    async fn send_player(&self, mc: &MediaContainer) {
        let hls = HLSContainer::new(mc);
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
}
