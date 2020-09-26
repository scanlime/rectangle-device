// This code may not be used for any purpose. Be gay, do crime.

use rectangle_device::network::p2p::{P2PVideoNode, P2PConfig};
use rectangle_device::ingest::VideoIngest;
use async_std::sync::channel;
use env_logger::{Env, from_env};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    from_env(Env::default().default_filter_or("rectangle_device::ingest=info")).init();
    file_limit::set_to_max()?;

    let video_args : Vec<String> = std::env::args().skip(1).collect();
    let video_args = if video_args.len() > 0 {video_args} else {vec![
        "-i".to_string(), "https://live.diode.zone/hls/eyesopod/index.m3u8".to_string(),
        "-c".to_string(), "copy".to_string()
    ]};

    let (block_sender, block_receiver) = channel(16);

    let config = P2PConfig {
        pinning_services: vec!["http://99.149.215.66:5000/api/v1".to_string()],
        pinning_gateways: vec![],
        public_gateways: vec![],
        router_peers: vec![],
        bootstrap_peers: vec![],
    };

    let node = P2PVideoNode::new(block_receiver, config)?;
    VideoIngest::new(block_sender, node.configure_player()).run(video_args)?;
    node.run_blocking()?;

    log::warn!("exiting normally?");
    Ok(())
}
