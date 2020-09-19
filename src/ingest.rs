// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::Sender;
use async_std::task;
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use mpeg2ts::time::ClockReference;
use std::io::Cursor;
use std::process::{Command, Stdio};
use std::time::Duration;
use libp2p::PeerId;
use libipld::Cid;
use crate::config;
use crate::blocks::{BlockUsage, BlockInfo, RawFileBlock};
use crate::container::media::{Segment, Container};
use crate::container::hls::HLSContainer;
use crate::container::html::HLSPlayer;

pub struct VideoIngest {
    pub block_sender: Sender<BlockInfo>,
    pub pin_sender: Sender<Cid>,
}

impl VideoIngest {
    pub fn run(self, args: Vec<String>, local_peer_id: &PeerId) {
        log::info!("ingest process starting, {:?}", args);

        // To do: Sandbox ffmpeg. gaol is promising but we'd need to stop using stdout.

        let mut command = Command::new("ffmpeg");
        command
            .arg("-nostats").arg("-nostdin")
            .arg("-loglevel").arg("error")
            .args(args)
            .arg("-c").arg("copy")
            .arg("-f").arg("stream_segment")
            .arg("-segment_format").arg("mpegts")
            .arg("-segment_wrap").arg("1")
            .arg("-segment_time").arg(config::SEGMENT_MIN_SEC.to_string())
            .arg("pipe:%d.ts");

        log::info!("using command: {:?}", command);

        let mpegts = command
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn().unwrap()
            .stdout.take().unwrap();

        let mut reader = TsPacketReader::new(mpegts);
        let mut segment_buffer = [0 as u8; config::SEGMENT_MAX_BYTES];
        let mut cursor = Cursor::new(&mut segment_buffer[..]);
        let mut container = Container { blocks: vec![] };
        let mut clock_latest: Option<ClockReference> = None;
        let mut clock_first: Option<ClockReference> = None;
        let mut segment_clock: Option<ClockReference> = None;
        let mut program_association_table: Option<TsPacket> = None;

        while let Some(packet) = reader.read_ts_packet().unwrap() {

            // Save a copy of the PAT (Program Association Table) and reinsert it at every segment
            if packet.header.pid.as_u16() == mpeg2ts::ts::Pid::PAT {
                program_association_table = Some(packet.clone());
            }

            // What would the segment size be if we output one right before 'packet'
            let segment_bytes = cursor.position() as usize;
            let segment_ticks = clock_latest.map_or(0, |c| c.as_u64()) - segment_clock.or(clock_first).map_or(0, |c| c.as_u64());
            let segment_sec = (segment_ticks as f32) * (1.0 / (ClockReference::RESOLUTION as f32));

            // To do: determining keyframes properly actually requires looking at the video
            // packet data. At the mpeg-ts layer there's a "random access indicator" which
            // seems to be vaguely useful but if you actually start playback there you're still
            // missing the mpeg-ts PAT and several other things. This code need to do
            // a much better job, but for now with ffmpeg's help the stream seems regular
            // enough that we can safely split it just before Pid(17) containing the
            // Service Description Table.
            let is_keyframe = packet.header.pid.as_u16() == 17;

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
                let segment = Segment {
                    cid: segment_file.block.cid.clone(),
                    bytes: segment_bytes,
                    duration: segment_sec,
                    sequence: container.blocks.len()
                };
                let usage = BlockUsage::VideoSegment(segment.sequence);
                task::block_on(self.block_sender.send(segment_file.use_as(usage)));

                // Add each block to a table of contents, which is sent less frequently
                container.blocks.push(segment);
                if container.blocks.len() % config::PUBLISH_INTERVAL == 0 {
                    task::block_on(async {
                        let hls = HLSContainer::new(&container);
                        let player = HLSPlayer::from_hls(&hls, &local_peer_id);
                        let player_cid = player.directory.block.cid.clone();

                        log::info!("PLAYER created ====> https://{}.{} ({} bytes)",
                            player_cid.to_string(),
                            config::IPFS_GATEWAY,
                            player.directory.total_size);

                        hls.send(&self.block_sender).await;
                        player.send(&self.block_sender).await;
                        self.pin_sender.send(player_cid).await;
                    });
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
        loop {
            task::block_on(async {
                let hls = HLSContainer::new(&container);
                let player = HLSPlayer::from_hls(&hls, &local_peer_id);
                let player_cid = player.directory.block.cid.clone();

                log::warn!("ingest stream ended, final PLAYER ====> https://{}.{} ({} bytes)",
                    player_cid.to_string(),
                    config::IPFS_GATEWAY,
                    player.directory.total_size);

                hls.send(&self.block_sender).await;
                player.send(&self.block_sender).await;
                self.pin_sender.send(player_cid).await;

                task::sleep(Duration::from_secs(60)).await;
            });
        }
    }
}
