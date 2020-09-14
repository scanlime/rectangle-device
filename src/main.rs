use multibase::Base;
use libipld::{block::Block, raw::RawCodec};
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
use std::io::Cursor;
use std::process::{Command, Stdio};

#[derive(NetworkBehaviour)]
struct BehaviourImpl {
    gossipsub: Gossipsub,
    identify: Identify,
    ping: Ping,
    kad: Kademlia<MemoryStore>,
    bitswap: Bitswap<Multihash>,
    mdns: Mdns,
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for BehaviourImpl {
    // Called when `identify` produces an event.
    fn inject_event(&mut self, event: IdentifyEvent) {
        println!("identify {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for BehaviourImpl {
    fn inject_event(&mut self, event: GossipsubEvent) {
        println!("gossipsub {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for BehaviourImpl {
    fn inject_event(&mut self, event: PingEvent) {
        println!("ping {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for BehaviourImpl {
    fn inject_event(&mut self, event: KademliaEvent) {
        println!("kad {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<BitswapEvent> for BehaviourImpl {
    fn inject_event(&mut self, event: BitswapEvent) {
        println!("bitswap {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for BehaviourImpl {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer, _) in list {
                    log::info!("mdns discovered {:?}", peer);
                    self.bitswap.connect(peer);
                }
            },
            MdnsEvent::Expired(_) => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    let transport = libp2p::build_development_transport(local_key.clone())?;
    let gossipsub_topic = gossipsub::Topic::new("rectangle-net".into());

    task::spawn_blocking(|| {
        // Get an mpeg transport stream of isopods
        let mpegts = Command::new("ffmpeg")
            .arg("-loglevel").arg("panic")
            .arg("-i").arg("https://live.diode.zone/hls/eyesopod/index.m3u8")
            .arg("-c").arg("copy")
            .arg("-f").arg("mpegts").arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn().unwrap()
            .stdout.take().unwrap();

        // Break it at points where the mpegts random access indicator is set,
        // with a preset minimum and maximum size for each segment.
        const SEGMENT_MIN : usize = 512*1024;
        const SEGMENT_MAX : usize = 1024*1024;

        let mut reader = TsPacketReader::new(mpegts);
        let mut buffer = [0 as u8; SEGMENT_MAX];
        let mut cursor = Cursor::new(&mut buffer[..]);

        while let Some(packet) = reader.read_ts_packet().unwrap() {
            let is_keyframe = packet.adaptation_field.as_ref().map_or(false, |a| a.random_access_indicator);
            let position = cursor.position() as usize;
            if (is_keyframe && position >= SEGMENT_MIN) || (position + TsPacket::SIZE > SEGMENT_MAX) {
                cursor.set_position(0);
                let segment = cursor.get_ref().get(0 .. position).unwrap();
                let block : Block<Multicodec, Multihash> = Block::encode(RawCodec, SHA2_256, segment).unwrap();
                let cid_str = block.cid.to_string_of_base(Base::Base32Lower).unwrap();

                println!("block size {} cid {}", segment.len(), cid_str);
            }
            let mut writer = TsPacketWriter::new(&mut cursor);
            writer.write_ts_packet(&packet).unwrap();
        }
    });

    let mut swarm = {
        let gossipsub_config = GossipsubConfigBuilder::new()
            .max_transmit_size(262144)
            .build();
        let mut behaviour = BehaviourImpl {
            gossipsub: Gossipsub::new(MessageAuthenticity::Signed(local_key.clone()), gossipsub_config),
            identify: Identify::new(
                "/ipfs/0.1.0".into(),
                "rectangle-device".into(),
                local_key.public(),
            ),
            ping: Ping::new(PingConfig::new()),
            bitswap: Bitswap::new(),
            kad: Kademlia::new(local_peer_id.clone(), MemoryStore::new(local_peer_id.clone())),
            mdns: Mdns::new()?,
        };
        behaviour.gossipsub.subscribe(gossipsub_topic.clone());
        Swarm::new(transport, behaviour, local_peer_id.clone())
    };

    Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse()?)?;

    task::block_on(async {
        loop {
            let event = swarm.next_event().await;
            log::trace!("{:?}", event);
            match event {
                SwarmEvent::NewListenAddr(addr) => {
                    println!("serving {}/p2p/{}", addr, local_peer_id);
                },
                _ => {}
            };
        }
    })
}
