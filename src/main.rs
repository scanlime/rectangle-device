// This code may not be used for any purpose. Be gay, do crime.

use rectangle_device::{config, ingest, network, pinner, warmer};
use async_std::sync::channel;
use async_std::task;
use env_logger::Env;
use std::error::Error;
use std::thread;

// Input, as a partial ffmpeg command line. Note that filenames here are parsed according to
// libavformat's protocol rules. This can be overridden by giving a command line.
fn default_args() -> Vec<String> {
    vec![
        "-i".to_string(), "https://live.diode.zone/hls/eyesopod/index.m3u8".to_string(),
        "-c".to_string(), "copy".to_string()
    ]
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::from_env(Env::default().default_filter_or("rectangle_device::ingest=info")).init();
    file_limit::set_to_max()?;

    let video_args : Vec<String> = std::env::args().skip(1).collect();

    let (block_sender, block_receiver) = channel(16);

    let config = P2PConfig {
        pinning_services: vec!["http://99.149.215.66:5000/api/v1".to_string()],
        pinning_gateways: vec![],
        public_gateways: vec![],
        router_peers: vec![],
        bootstrap_peers: vec![],
    };

    let node = network::P2PVideoNode::new(block_receiver, config)?;
    let ingest = ingest::VideoIngest::new(block_sender, node.configure_player());

    thread::Builder::new().name("vid-in".to_string()).spawn(move || {
        let args = if video_args.len() == 0 default_args() else video_args;
        ingest.run(args).unwrap();
    })?;

    task::Builder::new().name("p2p-node".to_string()).blocking(node);

    log::warn!("exiting normally?");
    Ok(())
}
