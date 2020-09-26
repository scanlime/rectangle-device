// This code may not be used for any purpose. Be gay, do crime.

#[macro_use]
extern crate clap;

use rectangle_device_network::p2p::{P2PVideoNode, P2PConfig};
use rectangle_device_media::ingest::VideoIngest;
use async_std::sync::channel;
use env_logger::{Env, from_env};
use clap::{App, ArgMatches};
use std::error::Error;

fn arg_values<S: AsRef<str>>(matches: &ArgMatches, name: S) -> Vec<String> {
    match matches.values_of(name) {
        Some(strs) => strs.map(|s| s.to_string()).collect(),
        None => Vec::new(),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let log_level = matches.value_of("log_level").unwrap();
    from_env(Env::default().default_filter_or(log_level)).init();
    file_limit::set_to_max()?;

    let config = P2PConfig {
        pinning_services: arg_values(&matches, "pinning_services"),
        pinning_gateways: arg_values(&matches, "pinning_gateways"),
        public_gateways: arg_values(&matches, "public_gateways"),
        router_peers: arg_values(&matches, "router_peers"),
        additional_peers: arg_values(&matches, "additional_peers"),
    };
    log::info!("{:?}", config);

    let (block_sender, block_receiver) = channel(16);
    let node = P2PVideoNode::new(block_receiver, config)?;

    let video_args = arg_values(&matches, "video_args");
    VideoIngest::new(block_sender, node.configure_player()).run(video_args)?;
    node.run_blocking()?;

    log::warn!("exiting normally?");
    Ok(())
}
