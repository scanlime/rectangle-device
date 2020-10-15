#[macro_use] extern crate clap;

use clap::{App, ArgMatches};
use env_logger::{from_env, Env};
use rectangle_device_media::{ingest::VideoIngest, MediaUpdateBus};
use rectangle_device_network::p2p::{Multiaddr, P2PConfig, P2PVideoNode, Url};
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

    let video_args = string_values(&matches, "video_args");
    let publish_interval = matches.value_of("publish_interval").unwrap();
    let publish_interval = publish_interval
        .parse::<humantime::Duration>()
        .expect("expected a valid `humantime`-compatible duration argument");

    let mub = MediaUpdateBus::new();
    let node = P2PVideoNode::new(&mub, config)?;
    let ingest = VideoIngest::new(&mub, publish_interval.into());
    ingest.run(video_args)?;

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
