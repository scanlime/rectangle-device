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
use crate::sandbox::runtime;

use crate::sandbox::types::{ImageDigest, SandboxError};
use std::process::{Command, Stdio};
use std::error::Error;

const TEMP_PREFIX : &'static str = "rectdev.";
const SOCKET_MOUNT : &'static str = "/rectdev";

pub struct SocketPool {

}

struct PoolItem {
    host_path: 
}


impl SocketPool {
    pub fn push(&mut self, name: &str) ->
    }
}

/*


struct SocketRing {
    dir: TempDir,
    paths: Vec<PathBuf>,
    mount_args: Vec<String>,
    listeners: Vec<UnixListener>,
    buffers: Vec<u8>,
}
*/
/*
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
