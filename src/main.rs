use async_std::sync::{channel, Sender, Receiver};
use async_std::task::{self, Poll, JoinHandle, Context};
use core::pin::Pin;
use env_logger::Env;
use futures::{Future, Stream};
use libipld::{cid::Cid, block::Block, Ipld, raw::RawCodec, codec_impl::Multicodec, pb::DagPbCodec};
use libipld::multihash::{Multihash, SHA2_256};
use libp2p_bitswap::{Bitswap, BitswapEvent};
use libp2p::{identity, PeerId, Swarm, NetworkBehaviour};
use libp2p::core::multiaddr::{Multiaddr, Protocol};
use libp2p::gossipsub::{self, Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, GossipsubEvent};
use libp2p::gossipsub::error::PublishError;
use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::kad::{self, Kademlia, KademliaEvent, KademliaConfig};
use libp2p::kad::record::store::{MemoryStore, MemoryStoreConfig};
use libp2p::mdns::{Mdns, MdnsEvent};
use libp2p::ping::{Ping, PingConfig, PingEvent};
use libp2p::swarm::{SwarmEvent, NetworkBehaviourEventProcess, NetworkBehaviour};
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use mpeg2ts::time::ClockReference;
use std::borrow::Cow;
use std::collections::{BTreeMap, VecDeque};
use std::convert::TryFrom;
use std::error::Error;
use std::io::Cursor;
use std::process::{Command, Stdio};

type BlockType = Block<Multicodec, Multihash>;
const IPFS_GATEWAY: &'static str = "ipfs.cf-ipfs.com";

struct VideoIngest {
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
    block_store: BTreeMap<Vec<u8>, BlockType>,
    #[behaviour(ignore)]
    send_queue: VecDeque<(PeerId, Cid)>
}

struct P2PVideoNode {
    gossipsub_topic: gossipsub::Topic,
    swarm: Swarm<P2PVideoBehaviour>
}

struct TableOfContents {
    blocks: Vec<(Cid, usize, f32)>,
}

impl TableOfContents {
    fn to_unixfs_block(&self) -> BlockType {

        let mut contents: Vec<Ipld> = vec![];
        let unixfs_dir: Vec<u8> = vec![8, 1];

        for (cid, bytes, _) in &self.blocks {
            let ts_filename = format!("{:05}.ts", contents.len() + 1);

            let mut pb_link = BTreeMap::<String, Ipld>::new();
            pb_link.insert("Hash".to_string(), cid.clone().into());
            pb_link.insert("Name".to_string(), ts_filename.into());
            pb_link.insert("Tsize".to_string(), (*bytes).into());

            contents.push(pb_link.into());
        }

        let mut pb_node = BTreeMap::<String, Ipld>::new();
        pb_node.insert("Links".to_string(), contents.clone().into());
        pb_node.insert("Data".to_string(), unixfs_dir.into());
        let contents_ipld: Ipld = pb_node.into();

        Block::encode(DagPbCodec, SHA2_256, &contents_ipld).unwrap()
    }

    async fn send(&self, block_sender: &Sender<BlockType>) {
        let block = self.to_unixfs_block();
        let cid_str = block.cid.to_string();
        block_sender.send(block).await;
        println!("https://{}.{}", cid_str, IPFS_GATEWAY);
    }
}

impl VideoIngest {
    fn spawn(self, args: Vec<String>) -> JoinHandle<()> {
        task::spawn_blocking(move || self.run(args))
    }

    fn run(self, args: Vec<String>) {
        const SEGMENT_MIN_BYTES : usize = 64*1024;
        const SEGMENT_MAX_BYTES : usize = 1024*1024;
        const SEGMENT_MIN_SEC : f32 = 1.5;
        const SEGMENT_MAX_SEC : f32 = 5.5;

        log::info!("ingest process starting, {:?}", args);

        let mpegts = Command::new("ffmpeg")
            .arg("-nostats").arg("-nostdin")
            .arg("-loglevel").arg("warning")
            .args(args)
            .arg("-c").arg("copy")
            .arg("-f").arg("mpegts").arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn().unwrap()
            .stdout.take().unwrap();

        let mut reader = TsPacketReader::new(mpegts);
        let mut segment_buffer = [0 as u8; SEGMENT_MAX_BYTES];
        let mut cursor = Cursor::new(&mut segment_buffer[..]);
        let mut contents = TableOfContents { blocks: vec![] };
        let mut clock_latest: Option<ClockReference> = None;
        let mut clock_first: Option<ClockReference> = None;
        let mut segment_clock: Option<ClockReference> = None;

        while let Some(packet) = reader.read_ts_packet().unwrap() {

            // What would the segment size be if we output one right before 'packet'
            let segment_bytes = cursor.position() as usize;
            let segment_ticks = clock_latest.map_or(0, |c| c.as_u64()) - segment_clock.or(clock_first).map_or(0, |c| c.as_u64());
            let segment_sec = (segment_ticks as f32) * (1.0 / (ClockReference::RESOLUTION as f32));

            // If the 'random access indicator' flag in the adaptation field is set,
            // treat this like a keyframe and prefer to start segments just before this packet.
            let is_keyframe = packet.adaptation_field.as_ref().map_or(false, |a| a.random_access_indicator);

            // This is the most recent timestamp we know about as of 'packet'
            if let Some(pcr) = packet.adaptation_field.as_ref().and_then(|a| a.pcr) {
                clock_latest = Some(pcr);
                if clock_first.is_none() {
                    clock_first = clock_latest;
                }
            }

            // Split on keyframes, but respect our hard limits on time and size
            if (is_keyframe && segment_bytes >= SEGMENT_MIN_BYTES && segment_sec >= SEGMENT_MIN_SEC) ||
               (segment_bytes + TsPacket::SIZE > SEGMENT_MAX_BYTES) ||
               (segment_sec >= SEGMENT_MAX_SEC) {

                cursor.set_position(0);
                let segment = cursor.get_ref().get(0..segment_bytes).unwrap();

                // Hash the video segment here, then send it to the other task for storage
                let block = Block::encode(RawCodec, SHA2_256, segment).unwrap();
                let cid = block.cid.clone();
                task::block_on(self.block_sender.send(block));

                // Add each block to a table of contents, which is sent less frequently
                contents.blocks.push((cid, segment_bytes, segment_sec));
                if contents.blocks.len() % 10 == 0 {
                    task::block_on(contents.send(&self.block_sender));
                }

                // This 'packet' will be the first in a new segment
                segment_clock = clock_latest;
            }

            let mut writer = TsPacketWriter::new(&mut cursor);
            writer.write_ts_packet(&packet).unwrap();
        }

        log::warn!("ingest stream ended");
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
                log::info!("received block {} {} {}", peer_id, cid.to_string(), data.len());
            },
            BitswapEvent::ReceivedWant(peer_id, cid, _) => {
                if self.block_store.contains_key(&cid.hash().to_bytes()) {
                    log::info!("peer {} wants our block {}", peer_id, cid.to_string());
                    self.send_queue.push_back((peer_id, cid));
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
                GossipsubConfigBuilder::new().build()
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
                MemoryStore::with_config(local_peer_id.clone(), kad_store_config()),
                kad_config.set_protocol_name(Cow::Borrowed(KAD_LAN)).clone()),
            kad_wan: Kademlia::with_config(
                local_peer_id.clone(),
                MemoryStore::with_config(local_peer_id.clone(), kad_store_config()),
                kad_config.set_protocol_name(Cow::Borrowed(KAD_WAN)).clone()),
            mdns: Mdns::new()?,
            peer_id: local_peer_id.clone(),
            block_store: BTreeMap::new(),
            send_queue: VecDeque::new(),
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

    fn store_block(&mut self, block: BlockType) {
        let cid_bytes = block.cid.to_bytes();
        let hash_bytes = block.cid.hash().to_bytes();
        let topic = self.gossipsub_topic.clone();

        match self.swarm.gossipsub.publish(&topic, cid_bytes.clone()) {
            Ok(()) => {},
            Err(PublishError::InsufficientPeers) => {},
            Err(err) => log::warn!("couldn't publish, {:?}", err)
        }

        self.swarm.kad_lan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
        self.swarm.kad_wan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
        self.swarm.block_store.insert(hash_bytes, block);
    }
}

impl Future for P2PVideoNode {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        loop {
            let queued_send = self.swarm.send_queue.pop_front();
            if let Some((peer_id, cid)) = queued_send {
                if let Some(block) = self.swarm.block_store.get(&cid.hash().to_bytes()) {
                    log::info!("SENDING block in response to want, {} -> {}", cid.to_string(), peer_id);
                    let block_data = block.data.clone();
                    self.swarm.bitswap.send_block(&peer_id, cid, block_data);
                }
            }

            let mut event_pending_counter = 2;
            let block_receiver_event = Pin::new(&mut self.swarm.block_receiver).poll_next(ctx);
            let network_event = unsafe { Pin::new_unchecked(&mut self.swarm.next_event()) }.poll(ctx);

            match block_receiver_event {
                Poll::Pending => event_pending_counter -= 1,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(block)) => {
                    let cid_str = block.cid.to_string();
                    let block_size = block.data.len();
                    self.store_block(block);
                    log::info!("ingest block size {} cid {}", block_size, cid_str);
                }
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
    VideoIngest {block_sender}.spawn(std::env::args().skip(1).collect());
    let node = P2PVideoNode::new(block_receiver)?;
    task::block_on(node);
    Ok(())
}
