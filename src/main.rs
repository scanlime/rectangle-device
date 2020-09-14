use multibase::Base;
use libipld::{cid::Cid, block::Block, raw::RawCodec};
use libipld::multihash::{Multihash, SHA2_256};
use libipld::codec_impl::Multicodec;
use libp2p::{identity, gossipsub, PeerId, Swarm, NetworkBehaviour};
use libp2p::swarm::{SwarmEvent, NetworkBehaviourEventProcess};
use libp2p::gossipsub::{Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, GossipsubEvent};
use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::kad::{Kademlia, KademliaEvent, record::store::MemoryStore};
use libp2p::ping::{Ping, PingConfig, PingEvent};
use libp2p_bitswap::{Bitswap, BitswapEvent};
use libp2p::mdns::{Mdns, MdnsEvent};
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use async_std::task;
use async_std::task::{Poll, JoinHandle, Context};
use async_std::sync::{channel, Sender, Receiver};
use std::io::Cursor;
use std::ops::DerefMut;
use std::error::Error;
use std::convert::TryFrom;
use core::pin::Pin;
use std::process::{Command, Stdio};
use futures::{Future, Stream};
use env_logger::Env;

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
    kad: Kademlia<MemoryStore>,
    bitswap: Bitswap<Multihash>,
    mdns: Mdns,

    #[behaviour(ignore)]
    block_receiver: Receiver<BlockType>
}

struct P2PVideoNode {
    local_peer_id: PeerId,
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

impl NetworkBehaviourEventProcess<IdentifyEvent> for P2PVideoBehaviour {
    fn inject_event(&mut self, event: IdentifyEvent) {
        log::trace!("identify {:?}", event);
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
        log::trace!("bitswap {:?}", event);
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
            kad: Kademlia::new(local_peer_id.clone(),
                MemoryStore::new(local_peer_id.clone())),
            mdns: Mdns::new()?,
            block_receiver,
        };

        behaviour.gossipsub.subscribe(gossipsub_topic.clone());

        let mut swarm = Swarm::new(transport, behaviour, local_peer_id.clone());
        Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(P2PVideoNode {
            local_peer_id,
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
            let block_receiver_event = Pin::new(&mut self.swarm.deref_mut().block_receiver).poll_next(ctx);
            let network_event = unsafe { Pin::new_unchecked(&mut self.swarm.next_event()) }.poll(ctx);

            match block_receiver_event {
                Poll::Pending => event_pending_counter -= 1,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(block)) => {
                    let block_size = block.data.len();
                    let cid_str = block.cid.to_string_of_base(Base::Base32Lower).unwrap();
                    let topic = self.gossipsub_topic.clone();
                    let publish_result = self.swarm.gossipsub.publish(&topic, block.cid.to_bytes());

                    log::info!("block size {} cid {} pub {:?}", block_size, cid_str, publish_result);
                },
            }

            match network_event {
                Poll::Pending => event_pending_counter -= 1,

                Poll::Ready(SwarmEvent::NewListenAddr(addr)) => {
                    log::info!("listening at {}/p2p/{}", addr, self.local_peer_id);
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
