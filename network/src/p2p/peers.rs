// This code may not be used for any purpose. Be gay, do crime.

use crate::p2p::config::P2PConfig;
use libp2p::PeerId;
use libp2p::core::multiaddr::Protocol;
use std::collections::{BTreeMap, BTreeSet};
use libp2p::Multiaddr;

pub struct ConfiguredPeers {
    map: BTreeMap<PeerId, ConfiguredPeerState>
}

struct ConfiguredPeerState {
    known_addresses: BTreeSet<Multiaddr>
}

impl ConfiguredPeers {
    pub fn new(config: &P2PConfig) -> Self {
        ConfiguredPeers {
            map: BTreeMap::new()
        }
    }
}

pub fn player_addr_filter(addr: &Multiaddr) -> bool {
    // Only give the player addresses that include a Wss protocol before the first p2p hop
    let mut result = false;
    for protocol in addr {
        use Protocol::*;
        match protocol {
            Http | Memory(_) | Unix(_) | Ws(_) => {
                // explicitly no.
                result = false;
                break;
            },
            P2p(_) | P2pCircuit | P2pWebRtcDirect | P2pWebRtcStar | P2pWebSocketStar => {
                // stop here
                break;
            },
            Wss(_) => {
                result = true;
            },
            _ => {}
        }
    }
    result
}

pub fn split_p2p_addr(addr: &Multiaddr) -> Option<(PeerId, Multiaddr)> {
    let mut addr_copy = addr.clone();
    if let Some(Protocol::P2p(hash)) = addr_copy.pop() {
        if let Ok(peer_id) = PeerId::from_multihash(hash) {
            Some((peer_id, addr_copy))
        } else {
            log::error!("address {} ignored because it has an invalid peer id hash", addr);
            None
        }
    } else {
        log::error!("address {} ignored because it does not end with a peer id hash", addr);
        None
    }
}
