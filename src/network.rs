// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use crate::warmer::Warmer;
use rectangle_device_blocks::{BlockUsage, BlockInfo};
use async_std::sync::Receiver;
use core::pin::Pin;
use std::cmp::Ordering;
use futures::{Future, Stream};
use libipld::cid::Cid;
use async_std::task::{Poll, Context};
use libipld::multihash::Multihash;
use libp2p_bitswap::{Bitswap, BitswapEvent};
use libp2p::{PeerId, Swarm, NetworkBehaviour};
use libp2p::core::multiaddr::{Multiaddr, Protocol};
use libp2p::gossipsub::{self, Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, GossipsubEvent};
use libp2p::gossipsub::error::PublishError;
use libp2p::identity::Keypair;
use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::kad::{self, Kademlia, KademliaEvent, KademliaConfig};
use libp2p::kad::record::store::{MemoryStore, MemoryStoreConfig};
use libp2p::mdns::{Mdns, MdnsEvent};
use libp2p::ping::{Ping, PingConfig, PingEvent};
use libp2p::swarm::{SwarmEvent, NetworkBehaviourEventProcess, NetworkBehaviour};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::error::Error;
use std::process::{Command, Stdio};

#[derive(Eq, Debug, Clone)]
struct BlockSendKey {
    pub usage: BlockUsage,
    pub cid: Cid,
    pub peer_id: PeerId,
}

impl Ord for BlockSendKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.usage.cmp(&other.usage)
    }
}

impl PartialOrd for BlockSendKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for BlockSendKey {
    fn eq(&self, other: &Self) -> bool {
        self.usage == other.usage
    }
}

pub struct P2PVideoNode {
    gossipsub_topic: gossipsub::Topic,
    pub swarm: Swarm<P2PVideoBehaviour>,
    warmer: Warmer,
}

#[derive(NetworkBehaviour)]
pub struct P2PVideoBehaviour {
    gossipsub: Gossipsub,
    identify: Identify,
    ping: Ping,
    kad_lan: Kademlia<MemoryStore>,
    kad_wan: Kademlia<MemoryStore>,
    bitswap: Bitswap<Multihash>,
    mdns: Mdns,

    #[behaviour(ignore)]
    peer_id: PeerId,
    #[behaviour(ignore)]
    block_receiver: Option<Receiver<BlockInfo>>,
    #[behaviour(ignore)]
    block_store: BTreeMap<Vec<u8>, BlockInfo>,
    #[behaviour(ignore)]
    blocks_to_send: BTreeSet<BlockSendKey>
}

impl P2PVideoBehaviour {
    pub fn add_router_address(&mut self, peer: &PeerId, address: Multiaddr) {
        self.kad_wan.add_address(peer, address.clone());
        let mut circuit = address.clone();
        circuit.push(Protocol::P2p(peer.clone().into()));
        circuit.push(Protocol::P2pCircuit);
        self.inject_new_listen_addr(&circuit);
        log::info!("listening via router {}/p2p/{}", circuit, self.peer_id);
    }
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Sent{..} => {},
            IdentifyEvent::Error{..} => {},
            IdentifyEvent::Received{ peer_id, info, observed_addr } => {
                log::trace!("identified peer {}, observing us as {}", peer_id, observed_addr);
                for addr in info.listen_addrs {
                    log::trace!("identified peer {}, listening at {}", peer_id, addr);
                    self.kad_wan.add_address(&peer_id, addr);
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Subscribed{..} => {},
            GossipsubEvent::Unsubscribed{..} => {},
            GossipsubEvent::Message(peer_id, _, message) => {
                if let Ok(cid) = Cid::try_from(message.data) {
                    log::info!("peer {} says {}", peer_id, cid.to_string());
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
            BitswapEvent::ReceivedCancel(_, _) => {},
            BitswapEvent::ReceivedBlock(peer_id, cid, data) => {
                log::debug!("received block {} {} {}", peer_id, cid.to_string(), data.len());
            },
            BitswapEvent::ReceivedWant(peer_id, cid, _) => {
                // Ignore blocks we don't have, sort blocks we do by their BlockUsage, and dedupe.
                if let Some(block_info) = self.block_store.get(&cid.hash().to_bytes()) {
                    let usage = block_info.usage.clone();
                    log::debug!("peer {} wants our block {}", peer_id, cid.to_string());
                    self.blocks_to_send.insert(BlockSendKey { usage, cid, peer_id });
                }
            },
        }
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Expired(_) => {},
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
    config.max_records = 128*1024;
    config.max_provided_keys = 128*1024;
    config
}

fn keypair_from_openssl_rsa() -> Result<Keypair, Box<dyn Error>> {
    // This is a temporary hack.
    // I'm using an RSA keypair because the new elliptic curve
    // based identity seems to break p2p-circuit routing in go-ipfs?

    log::info!("generating key pair");

    let mut genpkey = Command::new("openssl")
        .arg("genpkey").arg("-algorithm").arg("RSA")
        .arg("-pkeyopt").arg("rsa_keygen_bits:2048")
        .arg("-pkeyopt").arg("rsa_keygen_pubexp:65537")
        .stdout(Stdio::piped())
        .spawn().unwrap();

    let pkcs8 = Command::new("openssl")
        .arg("pkcs8").arg("-topk8").arg("-nocrypt")
        .arg("-outform").arg("der")
        .stdin(Stdio::from(genpkey.stdout.take().unwrap()))
        .output().unwrap();

    let mut data = pkcs8.stdout.as_slice().to_vec();
    Ok(Keypair::rsa_from_pkcs8(&mut data)?)
}

impl P2PVideoNode {
    pub fn new(block_receiver: Receiver<BlockInfo>, warmer: Warmer) -> Result<P2PVideoNode, Box<dyn Error>> {

        let local_key = keypair_from_openssl_rsa()?;
        let local_peer_id = PeerId::from(local_key.public());
        log::info!("local identity is {}", local_peer_id.to_string());

        let gossipsub_topic = gossipsub::Topic::new(config::GOSSIPSUB_TOPIC.into());
        let transport = libp2p::build_development_transport(local_key.clone())?;
        let mut kad_config : KademliaConfig = Default::default();
        const KAD_LAN : &[u8] = b"/ipfs/lan/kad/1.0.0";
        const KAD_WAN : &[u8] = b"/ipfs/wan/kad/1.0.0";

        let mut behaviour = P2PVideoBehaviour {
            gossipsub: Gossipsub::new(
                MessageAuthenticity::Signed(local_key.clone()),
                GossipsubConfigBuilder::new().build()
            ),
            identify: Identify::new(
                "/ipfs/0.1.0".into(),
                config::NETWORK_IDENTITY.into(),
                local_key.public(),
            ),
            ping: Ping::new(PingConfig::new()),
            bitswap: Bitswap::new(),
            kad_lan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::with_config(local_peer_id.clone(), kad_store_config()),
                kad_config.set_protocol_name(Cow::Borrowed(KAD_LAN)).clone()),
            kad_wan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::with_config(local_peer_id.clone(), kad_store_config()),
                kad_config.set_protocol_name(Cow::Borrowed(KAD_WAN)).clone()),
            mdns: Mdns::new()?,
            peer_id: local_peer_id.clone(),
            block_store: BTreeMap::new(),
            blocks_to_send: BTreeSet::new(),
            block_receiver: Some(block_receiver),
        };

        behaviour.add_router_address(&config::IPFS_ROUTER_ID.parse().unwrap(), config::IPFS_ROUTER_ADDR_TCP.parse().unwrap());
        behaviour.add_router_address(&config::IPFS_ROUTER_ID.parse().unwrap(), config::IPFS_ROUTER_ADDR_UDP.parse().unwrap());
        behaviour.kad_wan.bootstrap().unwrap();
        behaviour.gossipsub.subscribe(gossipsub_topic.clone());

        let mut swarm = Swarm::new(transport, behaviour, local_peer_id.clone());
        Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(P2PVideoNode {
            gossipsub_topic,
            warmer,
            swarm
        })
    }

    fn store_block(&mut self, block_info: BlockInfo) {
        let cid = block_info.block.cid.clone();
        let usage = block_info.usage.clone();

        let topic = self.gossipsub_topic.clone();
        match self.swarm.gossipsub.publish(&topic, cid.to_bytes()) {
            Ok(()) => {},
            Err(PublishError::InsufficientPeers) => {},
            Err(err) => log::warn!("couldn't publish, {:?}", err)
        }

        log::info!("{:?}", Swarm::network_info(&mut self.swarm));
        log::info!("stored {:7} bytes, {} {:?}",
            block_info.block.data.len(), block_info.block.cid.to_string(), usage);

        let hash_bytes = block_info.block.cid.hash().to_bytes();
        self.swarm.kad_lan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
        self.swarm.kad_wan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();

        self.swarm.block_store.insert(hash_bytes, block_info);

        self.warmup_block(cid, usage);
    }

    fn warmup_block(&self, cid: Cid, usage: BlockUsage) {
        let cid_str = cid.to_string();
        self.warmer.send(format!("http://{}/ipfs/{}", config::IPFS_LOCAL_GATEWAY, cid_str));
        match usage {
            BlockUsage::VideoSegment(_) => (),
            _ =>  self.warmer.send(format!("https://{}.ipfs.{}", cid_str, config::IPFS_GATEWAY))
        }
    }
}

impl Future for P2PVideoNode {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {

        // At most one bitswap sent block per wakeup for now, to keep networ from getting overwhelmed
        match self.swarm.blocks_to_send.iter().cloned().next() {
            None => {},
            Some(send_key) => {
                self.swarm.blocks_to_send.remove(&send_key);
                if let Some(block_info) = self.swarm.block_store.get(&send_key.cid.hash().to_bytes()) {
                    log::info!("SENDING block in response to want, {} {:?} -> {}",
                        send_key.cid.to_string(), send_key.usage, send_key.peer_id);
                    let peer_id = send_key.peer_id.clone();
                    let cid = send_key.cid.clone();
                    let data = block_info.block.data.clone();
                    self.swarm.bitswap.send_block(&peer_id, cid, data);
                }
            }
        }

        // Fully drain the block_receiver, store_block() should be fast
        loop {
            match self.swarm.block_receiver.take() {
                None => {
                    break;
                },
                Some(mut block_receiver) => {
                    let block_event = Pin::new(&mut block_receiver).poll_next(ctx);
                    match block_event {
                        Poll::Pending => {
                            self.swarm.block_receiver = Some(block_receiver);
                            break;
                        },
                        Poll::Ready(None) => {
                            drop(block_receiver);
                            break;
                        },
                        Poll::Ready(Some(block_info)) => {
                            self.swarm.block_receiver = Some(block_receiver);
                            self.store_block(block_info);
                        }
                    }
                }
            }
        }

        // Poll network until it's fully blocked on I/O
        loop {
            let network_event = unsafe { Pin::new_unchecked(&mut self.swarm.next_event()) }.poll(ctx);
            match network_event {
                Poll::Pending => {
                    return Poll::Pending;
                },
                Poll::Ready(SwarmEvent::NewListenAddr(addr)) => {
                    let peer_id = Swarm::local_peer_id(&self.swarm).clone();
                    log::info!("listening at {}/p2p/{}", addr, peer_id);
                    self.swarm.kad_lan.add_address(&peer_id, addr);
                },
                Poll::Ready(x) => {
                    log::trace!("network event {:?}", x);
                },
            }
        }
    }
}
