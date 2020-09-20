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

pub struct VideoIngest {
    block_sender: Sender<BlockInfo>,
    pin_sender: Sender<Cid>,
    hls_dist: HLSPlayerDist,
    local_peer_id: PeerId,
}

struct SocketRing {
    dir: TempDir,
    paths: Vec<PathBuf>,
    mount_args: Vec<String>,
    listeners: Vec<UnixListener>,
    buffers: Vec<u8>,
}

const NUM_SOCKETS : usize = 3;
const TEMP_PREFIX : &'static str = "rectangle-device.";
const SOCKET_MOUNT : &'static str = "/var/run/rectangle-device";
const PODMAN : &'static str = "podman";

impl SocketRing {
    fn new() -> SocketRing {
        let dir = tempfile::Builder::new().prefix(TEMP_PREFIX).tempdir().unwrap();

        let paths: Vec<PathBuf> = (0..NUM_SOCKETS).map(|id| {
            dir.path().join(format!("s{}", id))
        }).collect();

        let mount_args = (0..NUM_SOCKETS).map(|id| {
            format!("-v={}:{}/{}.ts", paths[id].to_str().unwrap(), SOCKET_MOUNT, id)
        }).collect();

        let listeners: Vec<UnixListener> = (&paths).iter().map(|path| {
            task::block_on(async {
                UnixListener::bind(path).await.unwrap()
            })
        }).collect();

        SocketRing { dir, paths, mount_args, listeners, buffers: vec![] }
    }

    fn recv(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        async_std::task::block_on(self.recv_task())
    }

    async fn recv_task(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(vec![])
    }

/*
        task::block_on(async {
            for (id, listener) in socket_listeners.into_iter().enumerate() {
                println!("{}", id);
                task::spawn(async move {
                    let mut incoming = listener.incoming();
                    while let Some(stream) = incoming.next().await {
                        if let Ok(mut stream) = stream {
                            println!("{} accepting", id);

                            let mut buf = [0; 1024*1024];
                            while let Ok(size) = stream.read(&mut buf).await {
                                println!("{} got {} bytes", id, size);
                            }
                        }
                    }
                });
            }
        });
*/
/*
        for (id, listener) in socket_listeners.into_iter().enumerate() {
            let sender = packet_sender.clone();
            std::thread::spawn(move || ts_packet_pump(id, listener, sender));
        }
        fn ts_packet_pump(id: usize, listener: UnixListener, sender: Sender<(usize, TsPacket)>) {
            for stream in listener.incoming() {
                let mut reader = TsPacketReader::new(stream.unwrap());
                while let Some(packet) = reader.read_ts_packet().unwrap() {
                    task::block_on(sender.send((id, packet)));
                };
            }
        }
*/
panic!("nope");
    //    Ok(())
    }
}

fn pull_image_if_necessary() {
    let mut podman_command = Command::new(PODMAN);
    let hash_exists_status = podman_command
        .arg("image").arg("exists")
        .arg(config::FFMPEG_CONTAINER_HASH)
        .status().unwrap();

    if !hash_exists_status.success() {
        log::warn!("downloading transcode container image");

        let mut podman_command = Command::new(PODMAN);
        let pulled_hash = String::from_utf8(
            podman_command
                .arg("pull")
                .arg(&config::FFMPEG_CONTAINER_NAME)
                .stderr(Stdio::inherit())
                .output().unwrap().stdout
            ).unwrap();
        let pulled_hash = pulled_hash.trim();
        assert_eq!(pulled_hash, config::FFMPEG_CONTAINER_HASH);
    }
}

impl VideoIngest {
    pub fn new(block_sender: Sender<BlockInfo>, pin_sender: Sender<Cid>, local_peer_id: PeerId) -> VideoIngest {
        let hls_dist = HLSPlayerDist::new();
        VideoIngest { block_sender, pin_sender, hls_dist, local_peer_id }
    }

    pub fn run(self, args: Vec<String>) {
        log::info!("ingest process starting, {:?}", args);

        pull_image_if_necessary();

        let socket_ring = SocketRing::new();
        let mut podman_command = Command::new(PODMAN);
        let run_command = podman_command
            .arg("run")
            .arg("-a").arg("stdout,stderr")
            .arg("--net=slirp4netns")
            .arg("--dns-search=.")
            .arg("--env-host=false")
            .arg("--read-only")
            .arg("--restart=no")
            .arg("--detach=false")
            .arg("--privileged=false")
            .args(socket_ring.mount_args.clone())
            .arg(config::FFMPEG_CONTAINER_HASH)
            .args(args)
            .arg("-nostats").arg("-nostdin")
            .arg("-loglevel").arg("error")
            .arg("-c").arg("copy")
            .arg("-f").arg("stream_segment")
            .arg("-segment_format").arg("mpegts")
            .arg("-segment_wrap").arg(NUM_SOCKETS.to_string())
            .arg("-segment_time").arg(config::SEGMENT_MIN_SEC.to_string())
            .arg(format!("unix://{}/%d.ts", SOCKET_MOUNT));

        log::info!("using command: {:?}", run_command);

        let mut _run_result = run_command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn().unwrap();

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

        while let Ok(buffer) = socket_ring.recv() {
            let mut reader = TsPacketReader::new(cursor::new(buffer))
        -        while let Some(packet) = reader.read_ts_packet().unwrap() {

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
