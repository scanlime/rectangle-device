use crate::p2p::{config::P2PConfig, peers::ConfiguredPeers, storage::BlockStore};
use libipld::{cid::Cid, multihash::Multihash};
use libp2p::{
    gossipsub::{self, Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity},
    identify::{Identify, IdentifyEvent},
    identity::Keypair,
    kad::{
        record::store::{MemoryStore, MemoryStoreConfig},
        Kademlia, KademliaConfig, KademliaEvent,
    },
    mdns::{Mdns, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour, PeerId,
};
use libp2p_bitswap::{Bitswap, BitswapEvent};
use std::{borrow::Cow, convert::TryFrom, error::Error};

const GOSSIPSUB_TOPIC: &'static str = "rectangle-net";

const NETWORK_IDENTITY: &'static str = "rectangle-device";
const NETWORK_PROTOCOL: &'static str = "/ipfs/0.1.0";

const KAD_LAN: &[u8] = b"/ipfs/lan/kad/1.0.0";
const KAD_WAN: &[u8] = b"/ipfs/wan/kad/1.0.0";

#[derive(NetworkBehaviour)]
pub struct P2PVideoBehaviour {
    pub gossipsub: Gossipsub,
    pub identify: Identify,
    pub ping: Ping,
    pub kad_lan: Kademlia<MemoryStore>,
    pub kad_wan: Kademlia<MemoryStore>,
    pub bitswap: Bitswap<Multihash>,
    pub mdns: Mdns,

    #[behaviour(ignore)]
    pub gossipsub_topic: gossipsub::Topic,

    #[behaviour(ignore)]
    pub block_store: BlockStore,

    #[behaviour(ignore)]
    pub configured_peers: ConfiguredPeers,
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Sent { .. } => {}
            IdentifyEvent::Error { .. } => {}
            IdentifyEvent::Received {
                peer_id,
                info,
                observed_addr,
            } => {
                log::trace!(
                    "identified peer {}, observing us as {}",
                    peer_id,
                    observed_addr
                );
                for addr in info.listen_addrs {
                    log::trace!("identified peer {}, listening at {}", peer_id, addr);

                    // Save additional addresses for our configured peers
                    self.configured_peers.save_known_address(&peer_id, &addr);

                    // Let the DHT know where we might be reachable
                    self.kad_wan.add_address(&peer_id, addr);
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Subscribed { .. } => {}
            GossipsubEvent::Unsubscribed { .. } => {}
            GossipsubEvent::Message(peer_id, _, message) => {
                if let Ok(cid) = Cid::try_from(message.data) {
                    log::debug!("peer {} says {}", peer_id, cid.to_string());
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: PingEvent) {
        log::trace!("ping {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: KademliaEvent) {
        log::trace!("kad {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<BitswapEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: BitswapEvent) {
        match event {
            BitswapEvent::ReceivedCancel(_, _) => {}
            BitswapEvent::ReceivedBlock(peer_id, cid, data) => {
                log::debug!(
                    "received block {} {} {}",
                    peer_id,
                    cid.to_string(),
                    data.len()
                );
            }
            BitswapEvent::ReceivedWant(peer_id, cid, _) => {
                // Ignore blocks we don't have, sort blocks we do by their BlockUsage, and dedupe.
                if let Some(block_info) = self.block_store.data.get(&cid.hash().to_bytes()) {
                    let usage = block_info.usage.clone();
                    log::debug!(
                        "peer {} wants our block {} {:?}",
                        peer_id,
                        cid.to_string(),
                        usage
                    );
                    self.block_store.enqueue_send(cid, peer_id, usage);
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Expired(_) => {}
            MdnsEvent::Discovered(list) => {
                for (peer, _) in list {
                    log::trace!("mdns discovered {:?}", peer);
                    self.bitswap.connect(peer);
                }
            }
        }
    }
}

fn kad_store_config() -> MemoryStoreConfig {
    let mut config: MemoryStoreConfig = Default::default();
    config.max_records = 128 * 1024;
    config.max_provided_keys = 128 * 1024;
    config
}

fn kad_protocol_config(name: &'static [u8]) -> KademliaConfig {
    let mut kad_config: KademliaConfig = Default::default();
    std::mem::take(kad_config.set_protocol_name(Cow::Borrowed(name)))
}

impl P2PVideoBehaviour {
    pub fn new(
        local_key: Keypair,
        local_peer_id: &PeerId,
        config: &P2PConfig,
    ) -> Result<P2PVideoBehaviour, Box<dyn Error>> {
        let public_key = local_key.public().clone();
        Ok(P2PVideoBehaviour {
            gossipsub_topic: gossipsub::Topic::new(GOSSIPSUB_TOPIC.into()),
            gossipsub: Gossipsub::new(
                MessageAuthenticity::Signed(local_key),
                GossipsubConfigBuilder::new().build(),
            ),
            identify: Identify::new(NETWORK_PROTOCOL.into(), NETWORK_IDENTITY.into(), public_key),
            ping: Ping::new(PingConfig::new()),
            bitswap: Bitswap::new(),
            kad_lan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::with_config(local_peer_id.clone(), kad_store_config()),
                kad_protocol_config(KAD_LAN),
            ),
            kad_wan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::with_config(local_peer_id.clone(), kad_store_config()),
                kad_protocol_config(KAD_WAN),
            ),
            mdns: Mdns::new()?,
            block_store: BlockStore::new(),
            configured_peers: ConfiguredPeers::new(config),
        })
    }
}
