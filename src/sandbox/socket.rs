// This code may not be used for any purpose. Be gay, do crime.

use async_std::stream::StreamExt;
use async_std::sync::{Sender, channel};
use async_std::os::unix::net::{UnixListener, Incoming};
use async_std::task;
use async_std::task::JoinHandle;
use async_std::io::ReadExt;
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use mpeg2ts::time::ClockReference;
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
use futures::future::BoxFuture;
use std::process::{Command, Stdio};
use std::error::Error;

const TEMP_PREFIX : &'static str = "rect-socket.";

pub struct SocketPool {
    pub paths: Vec<PathBuf>,
    servers: Vec<Server>,
    dir: TempDir,
}

struct Server {
    task: JoinHandle<()>,
}

#[derive(Clone, Debug)]
pub struct PoolEvent {
    id: usize,
    bytes: usize
}

impl SocketPool {
    pub async fn new(size: usize) -> Result<SocketPool, Box<dyn Error>> {

        let dir = tempfile::Builder::new().prefix(TEMP_PREFIX).tempdir()?;

        let paths: Vec<PathBuf> = (0..size).map(|n| dir.path().join(n.to_string())).collect();

        let mut servers: Vec<Server> = vec![];
        for path in &paths {
            servers.push(Server::new(path).await?);
        }

        Ok(SocketPool { dir, paths, servers })
    }

    pub async fn recv(&self) -> Option<PoolEvent> {

        task::sleep(Duration::from_secs(1)).await;

        Some(PoolEvent {
            id: 1234,
            bytes: 1234,
        })
    }
}

impl Server {
    async fn new(path: &PathBuf) -> Result<Server, Box<dyn Error>> {
        let listener = UnixListener::bind(path).await?;
        let task = task::spawn(Server::task(listener));
        Ok(Server { task })
    }

    async fn task(listener: UnixListener) {
/*
        let mut incoming = listener.incoming();

        while let Some(Ok(stream)) = incoming.next().await {
            println!("new stream");
            let mut stream = stream;

            let mut buf = [0 as u8; 1024*1024];
            while let Ok(size) = stream.read(&mut buf).await {
                println!("got {} bytes", size);
            }

            println!("end stream");
        }
*/
        println!("hi from task");

    }
}


/*

                            println!("{} accepting", id);
                        }
                    }
                });
            }
        });
        for (id, listener) in socket_listeners.into_iter().enumerate() {
            let sender = packet_sender.clone();
            std::thread::spawn(move || ts_packet_pump(id, listener, sender));
        }
*/
