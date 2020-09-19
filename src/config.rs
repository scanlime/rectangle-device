// This code may not be used for any purpose. Be gay, do crime.

use libp2p::PeerId;

// Input, as a partial ffmpeg command line. Note that filenames here are parsed according to
// libavformat's protocol rules. This can be overridden by giving a command line.
pub fn default_args() -> Vec<String> {
    vec![ "-i".to_string(), "https://live.diode.zone/hls/eyesopod/index.m3u8".to_string()]
}

// Network dependency: public HTTPS gateway
pub const IPFS_GATEWAY : &'static str = "cf-ipfs.com";

// Network dependency: go-ipfs relay server accessible over both TCP and WSS.
// This is used as a bootstrap for our local IPFS node, a p2p-circuit router, and a delegate for js-ipfs browser clients.
// In a production environment this could be a public or private machine with bandwidth but no storage
pub const IPFS_ROUTER_ID : &'static str = "QmPjtoXdQobBpWa2yS4rfmHVDoCbom2r2SMDTUa1Nk7kJ5";
pub const IPFS_ROUTER_ADDR_WSS : &'static str = "/dns4/ipfs.diode.zone/tcp/443/wss";
pub const IPFS_ROUTER_ADDR_TCP : &'static str = "/dns4/ipfs.diode.zone/tcp/4001";
pub const IPFS_ROUTER_ADDR_UDP : &'static str = "/dns4/ipfs.diode.zone/udp/4001/quic";

pub fn local_multiaddrs(local_peer_id: &PeerId) -> Vec<String> {
    vec![
        format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
            router_addr = IPFS_ROUTER_ADDR_TCP,
            router_id = IPFS_ROUTER_ID,
            local_id = local_peer_id),
        format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
            router_addr = IPFS_ROUTER_ADDR_UDP,
            router_id = IPFS_ROUTER_ID,
            local_id = local_peer_id),
        format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
            router_addr = IPFS_ROUTER_ADDR_WSS,
            router_id = IPFS_ROUTER_ID,
            local_id = local_peer_id)
    ]
}

// Optional network dependency: additional local IPFS gateway to use for the warmer pool
pub const IPFS_LOCAL_GATEWAY : &'static str = "99.149.215.66:8080";

// Optional network dependency: IPFS pinning services API, for requesting long-term storage of ingested video
// https://ipfs.github.io/pinning-services-api-spec/
pub const IPFS_PINNING_API : &'static str = "http://99.149.215.66:5000/api/v1";
pub const IPFS_PINNING_NAME : &'static str = "Experimental video stream from rectangle-device";

// Settings
pub const HLS_FILENAME : &'static str = "index.m3u8";
pub const JS_FILENAME : &'static str = "main.js";
pub const HLS_DIRECTORY : &'static str = "video";
pub const SEGMENT_MIN_BYTES : usize = 64*1024;
pub const SEGMENT_MAX_BYTES : usize = 1024*1024;
pub const SEGMENT_MIN_SEC : f32 = 2.0;
pub const SEGMENT_MAX_SEC : f32 = 5.0;
pub const PUBLISH_INTERVAL_SEC : u64 = 60;
