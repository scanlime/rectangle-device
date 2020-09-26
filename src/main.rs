// This code may not be used for any purpose. Be gay, do crime.

#[macro_use]
extern crate clap;

use rectangle_device_network::p2p::{P2PVideoNode, P2PConfig, Multiaddr, Url};
use rectangle_device_media::ingest::VideoIngest;
use async_std::sync::channel;
use env_logger::{Env, from_env};
use clap::{App, ArgMatches};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let log_level = matches.value_of("log_level").unwrap();
    from_env(Env::default().default_filter_or(log_level)).init();
    file_limit::set_to_max()?;

    let config = P2PConfig {
        pinning_services: url_values(&matches, "pinning_services"),
        pinning_gateways: url_values(&matches, "pinning_gateways"),
        public_gateways: url_values(&matches, "public_gateways"),
        router_peers: multiaddr_values(&matches, "router_peers"),
        additional_peers: multiaddr_values(&matches, "additional_peers"),
        listen_addrs: multiaddr_values(&matches, "listen_addrs"),
    };
    log::info!("{:?}", config);

    let node = P2PVideoNode::new(config)?;

    let video_args = string_values(&matches, "video_args");
// fix me: let's have a separate layer that connects stuff like p2pvideonode and videoingest, but isn't itself about either.
// probably just a channel for video blocks and/or playlists (so we don't have to send network addrs back to the ingest thread)

    VideoIngest::new(block_sender, node.configure_player()).run(video_args)?;

    node.run_blocking()?;
    panic!("network loop quit unexpectedly");
}

fn url_values<S: AsRef<str>>(matches: &ArgMatches, name: S) -> Vec<Url> {
    match matches.values_of(name) {
        Some(strs) => strs.map(|s| Url::parse(s).unwrap()).collect(),
        None => Vec::new(),
    }
}

fn string_values<S: AsRef<str>>(matches: &ArgMatches, name: S) -> Vec<String> {
    match matches.values_of(name) {
        Some(strs) => strs.map(|s| s.to_string()).collect(),
        None => Vec::new(),
    }
}

fn multiaddr_values<S: AsRef<str>>(matches: &ArgMatches, name: S) -> Vec<Multiaddr> {
    match matches.values_of(name) {
        Some(strs) => strs.map(|s| s.parse().unwrap()).collect(),
        None => Vec::new(),
    }
}
