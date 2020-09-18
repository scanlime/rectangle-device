// This code may not be used for any purpose. Be gay, do crime.

mod blocks;
mod config;
mod container;
mod ingest;
mod network;
mod pinner;

use async_std::sync::channel;
use async_std::task;
use env_logger::Env;
use libp2p::Swarm;
use std::error::Error;
use std::thread;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::from_env(Env::default().default_filter_or("rectangle_device=info")).init();
    let video_args = std::env::args().skip(1).collect();

    let (block_sender, block_receiver) = channel(32);
    let (pin_sender, pin_receiver) = channel(32);

    let node = network::P2PVideoNode::new(block_receiver)?;
    let local_peer_id = Swarm::local_peer_id(&node.swarm).clone();

    let pinner = pinner::Pinner {
        pin_receiver,
        local_multiaddrs: vec![
            format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
                router_addr = config::IPFS_ROUTER_ADDR_TCP,
                router_id = config::IPFS_ROUTER_ID,
                local_id = local_peer_id),
            format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
                router_addr = config::IPFS_ROUTER_ADDR_UDP,
                router_id = config::IPFS_ROUTER_ID,
                local_id = local_peer_id),
            format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
                router_addr = config::IPFS_ROUTER_ADDR_WSS,
                router_id = config::IPFS_ROUTER_ID,
                local_id = local_peer_id)
        ]
    };

    thread::Builder::new().name("pinner".to_string()).spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(pinner.task());
    })?;

    thread::Builder::new().name("vid-in".to_string()).spawn(move || {
        ingest::VideoIngest {
            block_sender,
            pin_sender
        }.run(video_args, &local_peer_id)
    })?;

    task::Builder::new().name("p2p-node".to_string()).blocking(node);

    Ok(())
}
