use async_std::sync::{channel, Sender, Receiver};
use async_std::task::{self, Poll, JoinHandle, Context};
use core::pin::Pin;
use env_logger::Env;
use futures::{Future, Stream};
use libipld::{cid::Cid, block::Block, raw::RawCodec};
use libipld::codec_impl::Multicodec;
use libipld::multihash::{Multihash, SHA2_256};
use libp2p_bitswap::{Bitswap, BitswapEvent};
use libp2p::{identity, PeerId, Swarm, NetworkBehaviour};
use libp2p::core::multiaddr::{Multiaddr, Protocol};
use libp2p::gossipsub::{self, Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, GossipsubEvent};
use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::kad::{self, Kademlia, KademliaEvent, KademliaConfig};
use libp2p::kad::record::store::MemoryStore;
use libp2p::mdns::{Mdns, MdnsEvent};
use libp2p::ping::{Ping, PingConfig, PingEvent};
use libp2p::swarm::{SwarmEvent, NetworkBehaviourEventProcess, NetworkBehaviour};
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use multibase::Base;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
use std::io::Cursor;
use std::process::{Command, Stdio};

type BlockType = Block<Multicodec, Multihash>;
const SEGMENT_MIN : usize = 512*1024;
const SEGMENT_MAX : usize = 1024*1024;

struct VideoIngest {
    src: &'static str,
    block_sender: Sender<BlockType>
}

#[derive(NetworkBehaviour)]
struct P2PVideoBehaviour {
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
    block_receiver: Receiver<BlockType>,
    #[behaviour(ignore)]
    block_store: HashMap<Vec<u8>, BlockType>
}

struct P2PVideoNode {
    gossipsub_topic: gossipsub::Topic,
    swarm: Swarm<P2PVideoBehaviour>
}

impl VideoIngest {
    fn spawn(self) -> JoinHandle<()> {
        task::spawn_blocking(move || self.run())
    }

    fn run(self) {
        let mpegts = Command::new("ffmpeg")
            .arg("-loglevel").arg("panic")
            .arg("-nostdin")
            .arg("-i").arg(self.src)
            .arg("-c").arg("copy")
            .arg("-f").arg("mpegts").arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn().unwrap()
            .stdout.take().unwrap();

        let mut reader = TsPacketReader::new(mpegts);
        let mut buffer = [0 as u8; SEGMENT_MAX];
        let mut cursor = Cursor::new(&mut buffer[..]);

        while let Some(packet) = reader.read_ts_packet().unwrap() {
            let is_keyframe = packet.adaptation_field.as_ref().map_or(false, |a| a.random_access_indicator);
            let position = cursor.position() as usize;
            if (is_keyframe && position >= SEGMENT_MIN) || (position + TsPacket::SIZE > SEGMENT_MAX) {
                cursor.set_position(0);
                let segment = cursor.get_ref().get(0 .. position).unwrap();
                let block = Block::encode(RawCodec, SHA2_256, segment).unwrap();
                task::block_on(self.block_sender.send(block));
            }
            let mut writer = TsPacketWriter::new(&mut cursor);
            writer.write_ts_packet(&packet).unwrap();
        }
    }
}

impl P2PVideoBehaviour {
    pub fn add_router_address(&mut self, peer: &PeerId, address: Multiaddr) {
        self.kad_wan.add_address(peer, address.clone());
        let mut circuit = address.clone();
        circuit.push(Protocol::P2p(peer.clone().into()));
        circuit.push(Protocol::P2pCircuit);
        self.inject_new_external_addr(&circuit);
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
                    let cid_str = cid.to_string_of_base(Base::Base32Lower).unwrap();
                    log::info!("peer {} says {}", peer_id, cid_str);
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
                log::info!("received block {} {} {}", peer_id, cid.to_string(), data.len());
            },
            BitswapEvent::ReceivedWant(peer_id, cid, _) => {
                if let Some(block) = self.block_store.get(&cid.hash().to_bytes()) {
                    log::info!("SENDING block in response to want, {} {}", peer_id, cid.to_string());
                    self.bitswap.send_block(&peer_id, cid, block.data.clone());
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

impl P2PVideoNode {
    fn new(block_receiver : Receiver<BlockType>) -> Result<P2PVideoNode, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        let gossipsub_topic = gossipsub::Topic::new("rectangle-net".into());
        let transport = libp2p::build_development_transport(local_key.clone())?;
        let mut kad_config : KademliaConfig = Default::default();
        const KAD_LAN : &[u8] = b"/ipfs/lan/kad/1.0.0";
        const KAD_WAN : &[u8] = b"/ipfs/wan/kad/1.0.0";

        let mut behaviour = P2PVideoBehaviour {
            gossipsub: Gossipsub::new(
                MessageAuthenticity::Signed(local_key.clone()),
                GossipsubConfigBuilder::new()
                    .max_transmit_size(262144)
                    .build()
            ),
            identify: Identify::new(
                "/ipfs/0.1.0".into(),
                "rectangle-device".into(),
                local_key.public(),
            ),
            ping: Ping::new(PingConfig::new()),
            bitswap: Bitswap::new(),
            kad_lan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::new(local_peer_id.clone()),
                kad_config.set_protocol_name(Cow::Borrowed(KAD_LAN)).clone()),
            kad_wan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::new(local_peer_id.clone()),
                kad_config.set_protocol_name(Cow::Borrowed(KAD_WAN)).clone()),
            mdns: Mdns::new()?,
            peer_id: local_peer_id.clone(),
            block_store: HashMap::new(),
            block_receiver,
        };

        behaviour.add_router_address(
            &"QmPfAR28wzi7s4BjeH5UVhJPQhGXBojsPe4bGDwZid15jq".parse().unwrap(),
            "/ip4/99.149.215.67/tcp/4001".parse().unwrap());

        behaviour.kad_wan.bootstrap().unwrap();
        behaviour.gossipsub.subscribe(gossipsub_topic.clone());

        let mut swarm = Swarm::new(transport, behaviour, local_peer_id.clone());
        Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(P2PVideoNode {
            gossipsub_topic,
            swarm
        })
    }
}

impl Future for P2PVideoNode {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        loop {
            let mut event_pending_counter = 2;
            let block_receiver_event = Pin::new(&mut self.swarm.block_receiver).poll_next(ctx);
            let network_event = unsafe { Pin::new_unchecked(&mut self.swarm.next_event()) }.poll(ctx);

            match block_receiver_event {
                Poll::Pending => event_pending_counter -= 1,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(block)) => {
                    let block_size = block.data.len();
                    let cid_str = block.cid.to_string_of_base(Base::Base32Lower).unwrap();
                    let cid_bytes = block.cid.to_bytes();
                    let hash_bytes = block.cid.hash().to_bytes();
                    let topic = self.gossipsub_topic.clone();

                    let publish_result = self.swarm.gossipsub.publish(&topic, cid_bytes.clone());
                    self.swarm.kad_lan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
                    self.swarm.kad_wan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
                    self.swarm.block_store.insert(hash_bytes, block);

                    log::info!("ingest block size {} cid {} pub {:?}", block_size, cid_str, publish_result);
                },
            }

            match network_event {
                Poll::Pending => event_pending_counter -= 1,

                Poll::Ready(SwarmEvent::NewListenAddr(addr)) => {
                    let peer_id = Swarm::local_peer_id(&self.swarm).clone();
                    log::info!("listening at {}/p2p/{}", addr, peer_id);
                    self.swarm.kad_lan.add_address(&peer_id, addr);
                },

                Poll::Ready(x) => {
                    log::trace!("network event {:?}", x);
                },
            }

            if event_pending_counter == 0 {
                return Poll::Pending;
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::from_env(Env::default().default_filter_or("rust_ipfs_toy=info")).init();
    let (block_sender, block_receiver) = channel(32);
    VideoIngest {block_sender, src: "https://live.diode.zone/hls/eyesopod/index.m3u8"}.spawn();
    let node = P2PVideoNode::new(block_receiver)?;
    task::block_on(node);
    Ok(())
}
