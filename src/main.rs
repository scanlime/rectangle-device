// This code may not be used for any purpose. Be gay, do crime.

mod blocks;
mod config;
mod container;
mod ingest;
mod network;
mod pinner;
mod warmer;

use async_std::sync::channel;
use async_std::task;
use env_logger::Env;
use libp2p::Swarm;
use std::error::Error;
use std::thread;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::from_env(Env::default().default_filter_or("rectangle_device=info")).init();
    let video_args : Vec<String> = std::env::args().skip(1).collect();

    let (block_sender, block_receiver) = channel(64);
    let (pin_sender, pin_receiver) = channel(128);

    let warmer = warmer::Warmer::new();
    let node = network::P2PVideoNode::new(block_receiver, warmer.clone())?;
    let local_peer_id = Swarm::local_peer_id(&node.swarm).clone();
    let local_multiaddrs = config::local_multiaddrs(&local_peer_id);

    let ingest = ingest::VideoIngest::new(block_sender, pin_sender, local_peer_id.clone());
    let pinner = pinner::Pinner { pin_receiver, local_multiaddrs };

    thread::Builder::new().name("warmer".to_string()).spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(warmer.task());
    })?;

    thread::Builder::new().name("pinner".to_string()).spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(pinner.task());
    })?;

    thread::Builder::new().name("vid-in".to_string()).spawn(move || {
        ingest.run(if video_args.len() == 0 {
            config::default_args()
        } else {
            video_args
        });
    })?;

    task::Builder::new().name("p2p-node".to_string()).blocking(node);

    Ok(())
}
