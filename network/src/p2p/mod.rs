// This code may not be used for any purpose. Be gay, do crime.

mod behaviour;
mod peers;
mod storage;
mod config;
mod node;

pub use crate::p2p::node::P2PVideoNode;
pub use crate::p2p::config::P2PConfig;

pub use libp2p::Multiaddr;
pub use url::Url;
