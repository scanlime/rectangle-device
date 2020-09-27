// This code may not be used for any purpose. Be gay, do crime.

use crate::p2p::config::P2PConfig;
use libp2p::PeerId;
use libp2p::core::multiaddr::Protocol;
use std::collections::{BTreeMap, BTreeSet};
use libp2p::Multiaddr;

pub struct ConfiguredPeers {
    map: BTreeMap<PeerId, ConfiguredPeerState>
}

impl ConfiguredPeers {
    pub fn new(config: &P2PConfig) -> Self {
        let mut peers = ConfiguredPeers {
            map: BTreeMap::new()
        };

        for addr in &config.router_peers {
            if let Some(parsed) = PeerAddr::parse(addr) {
                peers.save_new_peer(&parsed.peer_id, PeerUsage::CircuitRouterAndContentDelegate);
                peers.save_known_address(&parsed.peer_id, &parsed.addr);
            }
        }

        for addr in &config.additional_peers {
            if let Some(parsed) = PeerAddr::parse(addr) {
                peers.save_new_peer(&parsed.peer_id, PeerUsage::BootstrapOnly);
                peers.save_known_address(&parsed.peer_id, &parsed.addr);
            }
        }

        peers
    }

    fn save_new_peer(&mut self, peer_id: &PeerId, usage: PeerUsage) {
        self.map.insert(peer_id.clone(), ConfiguredPeerState::new(usage));
    }

    pub fn save_known_address(&mut self, peer_id: &PeerId, addr: &Multiaddr) {
        if let Some(state) = self.map.get_mut(peer_id) {
            state.save_known_address(addr.clone());
        }
    }

    pub fn node_bootstrap<'a>(&'a self) -> impl IntoIterator<Item = PeerAddr> + 'a {
        self.map.iter().map(|(peer_id, peer_state)| {
            peer_state.known_addresses.iter().map(move |addr| {
                PeerAddr {
                    peer_id: peer_id.clone(),
                    addr: addr.clone()
                }
            })
        }).flatten()
    }

    pub fn node_circuit_addrs<'a>(&'a self) -> impl IntoIterator<Item = Multiaddr> + 'a {
        self.map.iter().filter(move |(_, peer_state)| {
            match &peer_state.usage {
                PeerUsage::BootstrapOnly => false,
                PeerUsage::CircuitRouterAndContentDelegate => true
            }
        }).map(move |(peer_id, peer_state)| {
            peer_state.p2p_circuit_addresses(peer_id)
        }).flatten()
    }

    pub fn player_delegates<'a>(&'a self) -> impl IntoIterator<Item = Multiaddr> + 'a {
        self.map.iter().filter(move |(_, peer_state)| {
            match &peer_state.usage {
                PeerUsage::BootstrapOnly => false,
                PeerUsage::CircuitRouterAndContentDelegate => true
            }
        }).map(move |(peer_id, peer_state)| {
            peer_state.p2p_wss_addresses(peer_id)
        }).flatten()
    }

    pub fn player_bootstrap<'a>(&'a self) -> impl IntoIterator<Item = Multiaddr> + 'a {
        self.map.iter().map(move |(peer_id, peer_state)| {
            peer_state.p2p_wss_addresses(peer_id)
        }).flatten()
    }
}

pub struct PeerAddr {
    pub peer_id: PeerId,
    pub addr: Multiaddr
}

impl PeerAddr {
    pub fn parse(addr: &Multiaddr) -> Option<PeerAddr> {
        let mut addr_copy = addr.clone();
        if let Some(Protocol::P2p(hash)) = addr_copy.pop() {
            if let Ok(peer_id) = PeerId::from_multihash(hash) {
                Some(PeerAddr { peer_id, addr: addr_copy })
            } else {
                log::error!("address {} ignored because it has an invalid peer id hash", addr);
                None
            }
        } else {
            log::error!("address {} ignored because it does not end with a peer id hash", addr);
            None
        }
    }
}

enum PeerUsage {
    CircuitRouterAndContentDelegate,
    BootstrapOnly,
}

struct ConfiguredPeerState {
    usage: PeerUsage,
    known_addresses: BTreeSet<Multiaddr>
}

impl ConfiguredPeerState {
    fn new(usage: PeerUsage) -> ConfiguredPeerState {
        ConfiguredPeerState {
            known_addresses: BTreeSet::new(),
            usage,
        }
    }

    fn save_known_address(&mut self, addr: Multiaddr) {
        self.known_addresses.insert(addr);
    }

    fn p2p_addresses<'a>(&'a self, peer_id: &'a PeerId) -> impl IntoIterator<Item = Multiaddr> + 'a {
        self.known_addresses.iter().map(move |addr| {
            let mut addr = addr.clone();
            addr.push(Protocol::P2p(peer_id.clone().into()));
            addr
        })
    }

    fn p2p_circuit_addresses<'a>(&'a self, peer_id: &'a PeerId) -> impl IntoIterator<Item = Multiaddr> + 'a {
        self.p2p_addresses(peer_id).into_iter().map(move |mut addr| {
            addr.push(Protocol::P2pCircuit);
            addr
        })
    }

    fn p2p_wss_addresses<'a>(&'a self, peer_id: &'a PeerId) -> impl IntoIterator<Item = Multiaddr> + 'a {
        self.p2p_addresses(peer_id).into_iter().filter(|addr| {
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
        })
    }
}
