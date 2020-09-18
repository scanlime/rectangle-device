use async_std::sync::{channel, Sender, Receiver};
use async_std::task::{self, Poll, Context};
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
use m3u8_rs::playlist::{MediaPlaylist, MediaSegment, MediaPlaylistType};
use mpeg2ts::ts::{TsPacket, TsPacketReader, ReadTsPacket, TsPacketWriter, WriteTsPacket};
use mpeg2ts::time::ClockReference;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::error::Error;
use std::io::Cursor;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::thread;
use serde::{Deserialize, Serialize};

// Network dependency: public HTTPS gateway
const IPFS_GATEWAY : &'static str = "ipfs.cf-ipfs.com";

// Network dependency: hls player bundle pinned on IPFS.
// This could be served anywhere it's convenient. This CID is included in the final video bundle,
// so it will be automatically replicated by the pinning service.
// https://github.com/scanlime/hls-ipfs-player
const IPFS_PLAYER_CID : &'static str = "bafybeihjynl6i7ee3eimzuhy2vzm72utuwdiyzfkvhyiadwtl6mtgqcbzq";
const IPFS_PLAYER_NAME : &'static str = "hls-ipfs-player.js";
const IPFS_PLAYER_SIZE : usize = 2053462;

// Network dependency: go-ipfs relay server accessible over both TCP and WSS.
// This is used as a bootstrap for our local IPFS node, a p2p-circuit router, and a delegate for js-ipfs browser clients.
// In a production environment this could be a public or private machine with bandwidth but no storage
const IPFS_ROUTER_ID : &'static str = "QmPjtoXdQobBpWa2yS4rfmHVDoCbom2r2SMDTUa1Nk7kJ5";
const IPFS_ROUTER_ADDR_WSS : &'static str = "/dns4/ipfs.diode.zone/tcp/443/wss";
const IPFS_ROUTER_ADDR_TCP : &'static str = "/dns4/ipfs.diode.zone/tcp/4001";
const IPFS_ROUTER_ADDR_UDP : &'static str = "/dns4/ipfs.diode.zone/udp/4001/quic";

// Network dependency: IPFS pinning services API, for requesting long-term storage of ingested video
// https://ipfs.github.io/pinning-services-api-spec/
const IPFS_PINNING_API : &'static str = "http://99.149.215.66:5000/api/v1";
const IPFS_PINNING_NAME : &'static str = "Experimental video stream from rectangle-device";

// Settings
const HLS_FILENAME : &'static str = "index.m3u8";
const HLS_DIRECTORY : &'static str = "video";
const SEGMENT_MIN_BYTES : usize = 64*1024;
const SEGMENT_MAX_BYTES : usize = 1024*1024;
const SEGMENT_MIN_SEC : f32 = 2.0;
const SEGMENT_MAX_SEC : f32 = 5.0;
const PUBLISH_INTERVAL : usize = 50;

type BlockType = Block<Multicodec, Multihash>;

struct BlockInfo {
    block: BlockType,
    usage: BlockUsage,
}

struct VideoIngest {
    block_sender: Sender<BlockInfo>,
    pin_sender: Sender<Cid>,
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
    block_receiver: Receiver<BlockInfo>,
    #[behaviour(ignore)]
    block_store: BTreeMap<Vec<u8>, BlockInfo>,
    #[behaviour(ignore)]
    blocks_to_send: BTreeSet<BlockSendKey>
}

struct P2PVideoNode {
    gossipsub_topic: gossipsub::Topic,
    swarm: Swarm<P2PVideoBehaviour>
}

struct Pinner {
    pin_receiver: Receiver<Cid>,
    local_multiaddrs: Vec<String>
}

struct VideoContainer {
    blocks: Vec<(Cid, usize, f32)>,
}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq, Clone)]
enum BlockUsage {
    // We try to send blocks in the same order listed here
    PlayerDirectory(usize),
    Player(usize),
    VideoDirectory(usize),
    Playlist(usize),
    VideoSegment(usize),
}

#[derive(Eq, Debug, Clone)]
struct BlockSendKey {
    usage: BlockUsage,
    cid: Cid,
    peer_id: PeerId,
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

fn make_pb_link(cid: Cid, size: usize, name: String) -> Ipld {
    let mut pb_link = BTreeMap::<String, Ipld>::new();
    pb_link.insert("Hash".to_string(), cid.clone().into());
    pb_link.insert("Name".to_string(), name.into());
    pb_link.insert("Tsize".to_string(), size.into());
    pb_link.into()
}

fn make_pb_node(links: Vec<Ipld>, data: Vec<u8>) -> Ipld {
    let mut pb_node = BTreeMap::<String, Ipld>::new();
    pb_node.insert("Links".to_string(), links.into());
    pb_node.insert("Data".to_string(), data.into());
    pb_node.into()
}

fn make_raw_block(data: &[u8]) -> BlockType {
    Block::encode(RawCodec, SHA2_256, data).unwrap()
}

fn make_unixfs_directory(links: Vec<Ipld>) -> Ipld {
    const PBTAG_TYPE: u8 = 8;
    const TYPE_DIRECTORY: u8 = 1;
    make_pb_node(links, vec![PBTAG_TYPE, TYPE_DIRECTORY])
}

fn make_directory_block(links: Vec<Ipld>) -> BlockType {
    let ipld = make_unixfs_directory(links);
    Block::encode(DagPbCodec, SHA2_256, &ipld).unwrap()
}

#[derive(Deserialize, Serialize, Debug)]
struct APIPin {
    cid: String,
    name: String,
    origins: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct APIPinStatus {
    id: String,
    status: String,
    created: String,
    pin: APIPin,
    delegates: Vec<String>,
}

impl Pinner {
    async fn task(self) {
        let client = reqwest::Client::new();
        let mut id = None;

        loop {
            let cid = self.pin_receiver.recv().await.unwrap();
            let pin = APIPin {
                cid: cid.to_string(),
                name: IPFS_PINNING_NAME.to_string(),
                origins: self.local_multiaddrs.clone()
            };

            let url = match &id {
                None => format!("{}/pins", IPFS_PINNING_API),
                Some(id) => format!("{}/pins/{}", IPFS_PINNING_API, id)
            };

            let result = async {
                let result = client.post(&url).json(&pin).send().await?;
                let status: APIPinStatus = result.json().await?;
                log::info!("pinning api at {} says {:?}", url, status);
                Result::<String, Box<dyn Error>>::Ok(status.id)
            }.await;

            id = match result {
                Err(err) => {
                    log::error!("pinning api error, {}", err);
                    None
                },
                Ok(new_id) => Some(new_id),
            };
        }
    }
}

impl VideoContainer {
    fn make_playlist(&self, links: &mut Vec<Ipld>, total_size: &mut usize) -> BlockInfo {
        let mut segments = vec![];
        let mut segment_links = vec![];

        for (cid, segment_bytes, segment_sec) in &self.blocks {
            let filename = format!("s{:05}.ts", segments.len());
            *total_size += *segment_bytes;
            segment_links.push(make_pb_link(cid.clone(), *segment_bytes, filename.clone()));
            segments.push(MediaSegment {
                uri: filename.into(),
                duration: *segment_sec,
                ..Default::default()
            });
        }

        // https://tools.ietf.org/html/rfc8216
        let playlist = MediaPlaylist {
            version: 3,
            target_duration: SEGMENT_MAX_SEC,
            // To Do: Want to set this to true but it woud be a lie until we can split h264 too
            independent_segments: false,
            media_sequence: 0,
            playlist_type: Some(MediaPlaylistType::Vod),
            end_list: true,
            segments,
            ..Default::default()
        };

        let mut data: Vec<u8> = Vec::new();
        playlist.write_to(&mut data).unwrap();

        let block = make_raw_block(&data);
        *total_size += block.data.len();
        links.push(make_pb_link(block.cid.clone(), block.data.len(), HLS_FILENAME.to_string()));

        // Add the segment links after the playlist link, in case that helps clients find it quicker
        links.append(&mut segment_links);

        BlockInfo {
            block: block,
            usage: BlockUsage::Playlist(self.blocks.len())
        }
    }

    fn make_player(&self, links: &mut Vec<Ipld>, total_size: &mut usize, hls_cid: &Cid, hls_size: usize, local_peer_id: &PeerId) -> BlockInfo {
        let html_data = format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=100.0, minimum-scale=1.0" />
    <link rel="icon" href="data:," />
    <script src="https://{player:}.{gateway:}/"></script>
    <style>body {{ background: #000; margin: 0; }} video {{ position: absolute; width: 100%; height: 100%; left: 0; top: 0; }}</style>
</head>
<body>
    <video muted controls
        data-ipfs-src="{hls_cid:}/{hls_name:}"
        data-ipfs-delegates="{router_addr:}/p2p/{router_id:}"
        data-ipfs-bootstrap="{router_addr:}/p2p/{router_id:} {router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id}"
    ></video>
</body>
</html>
"#,
            player = IPFS_PLAYER_CID,
            gateway = IPFS_GATEWAY,
            router_addr = IPFS_ROUTER_ADDR_WSS,
            router_id = IPFS_ROUTER_ID,
            hls_cid = hls_cid.to_string(),
            hls_name = HLS_FILENAME,
            local_id = local_peer_id.to_string());

        let block = make_raw_block(&html_data.as_bytes());
        links.push(make_pb_link(block.cid.clone(), block.data.len(), "index.html".to_string()));
        links.push(make_pb_link(IPFS_PLAYER_CID.parse().unwrap(), IPFS_PLAYER_SIZE, IPFS_PLAYER_NAME.to_string()));
        links.push(make_pb_link(hls_cid.clone(), hls_size, HLS_DIRECTORY.to_string()));
        *total_size += block.data.len() + IPFS_PLAYER_SIZE + hls_size;

        BlockInfo {
            block: block,
            usage: BlockUsage::Player(self.blocks.len()),
        }
    }

    async fn send_hls_directory(&self, block_sender: &Sender<BlockInfo>) -> (Cid, usize) {
        let mut links = vec![];
        let mut total_size = 0;
        block_sender.send(self.make_playlist(&mut links, &mut total_size)).await;

        let dir_block = make_directory_block(links);
        let dir_cid = dir_block.cid.clone();
        total_size += dir_block.data.len();
        block_sender.send(BlockInfo {
            block: dir_block,
            usage: BlockUsage::VideoDirectory(self.blocks.len()),
        }).await;

        (dir_cid, total_size)
    }

    async fn send_player_directory(&self, block_sender: &Sender<BlockInfo>, local_peer_id: &PeerId) -> (Cid, usize) {
        let mut links = vec![];
        let mut total_size = 0;

        // Create and send the video + playlist in HLS format
        let (hls_cid, hls_size) = self.send_hls_directory(block_sender).await;

        // Create and send an HTML player that references the video
        block_sender.send(self.make_player(&mut links, &mut total_size, &hls_cid, hls_size, local_peer_id)).await;

        // The player won't care about this at all, but let's try referencing the video segments
        // from the player directory, to give the pinning service a head start on locating new Cids.
        // We use the links generated here but we don't need to send the block, as it's a repeat.
        // This also ends up double-counting the size of the video, since we list it twice, even
        // though nobody is duplicating the actual video data.
        self.make_playlist(&mut links, &mut total_size);

        let dir_block = make_directory_block(links);
        let dir_cid = dir_block.cid.clone();
        total_size += dir_block.data.len();
        block_sender.send(BlockInfo {
            block: dir_block,
            usage: BlockUsage::PlayerDirectory(self.blocks.len()),
        }).await;

        (dir_cid, total_size)
    }
}

impl VideoIngest {
    fn run(self, args: Vec<String>, local_peer_id: &PeerId) {
        log::info!("ingest process starting, {:?}", args);

        // To do: Sandbox ffmpeg. gaol is promising but we'd need to stop using stdout.

        let mut command = Command::new("ffmpeg");
        command
            .arg("-nostats").arg("-nostdin")
            .arg("-loglevel").arg("error")
            .args(args)
            .arg("-c").arg("copy")
            .arg("-f").arg("stream_segment")
            .arg("-segment_format").arg("mpegts")
            .arg("-segment_wrap").arg("1")
            .arg("-segment_time").arg(SEGMENT_MIN_SEC.to_string())
            .arg("pipe:%d.ts");

        log::info!("using command: {:?}", command);

        let mpegts = command
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn().unwrap()
            .stdout.take().unwrap();

        let mut reader = TsPacketReader::new(mpegts);
        let mut segment_buffer = [0 as u8; SEGMENT_MAX_BYTES];
        let mut cursor = Cursor::new(&mut segment_buffer[..]);
        let mut container = VideoContainer { blocks: vec![] };
        let mut clock_latest: Option<ClockReference> = None;
        let mut clock_first: Option<ClockReference> = None;
        let mut segment_clock: Option<ClockReference> = None;
        let mut program_association_table: Option<TsPacket> = None;

        while let Some(packet) = reader.read_ts_packet().unwrap() {

            // Save a copy of the PAT (Program Association Table) and reinsert it at every segment
            if packet.header.pid.as_u16() == mpeg2ts::ts::Pid::PAT {
                program_association_table = Some(packet.clone());
            }

            // What would the segment size be if we output one right before 'packet'
            let segment_bytes = cursor.position() as usize;
            let segment_ticks = clock_latest.map_or(0, |c| c.as_u64()) - segment_clock.or(clock_first).map_or(0, |c| c.as_u64());
            let segment_sec = (segment_ticks as f32) * (1.0 / (ClockReference::RESOLUTION as f32));

            // To do: determining keyframes properly actually requires looking at the video
            // packet data. At the mpeg-ts layer there's a "random access indicator" which
            // seems to be vaguely useful but if you actually start playback there you're still
            // missing the mpeg-ts PAT and several other things. This code need to do
            // a much better job, but for now with ffmpeg's help the stream seems regular
            // enough that we can safely split it just before Pid(17) containing the
            // Service Description Table.
            let is_keyframe = packet.header.pid.as_u16() == 17;

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
                let block = make_raw_block(segment);
                let cid = block.cid.clone();
                task::block_on(self.block_sender.send(BlockInfo {
                    block,
                    usage: BlockUsage::VideoSegment(container.blocks.len()),
                }));

                // Add each block to a table of contents, which is sent less frequently
                container.blocks.push((cid, segment_bytes, segment_sec));
                if container.blocks.len() % PUBLISH_INTERVAL == 0 {
                    task::block_on(async {
                        let (player_cid, total_size) = container.send_player_directory(&self.block_sender, &local_peer_id).await;
                        log::info!("PLAYER created ====> https://{}.{} ({} bytes)", player_cid.to_string(), IPFS_GATEWAY, total_size);
                        self.pin_sender.send(player_cid).await;
                    });
                }

                // Each segment starts with a PAT so the other packets can be identified
                if let Some(pat) = &program_association_table {
                    TsPacketWriter::new(&mut cursor).write_ts_packet(pat).unwrap();
                }

                // This 'packet' will be the first in a new segment after the PAT
                segment_clock = clock_latest;
            }

            TsPacketWriter::new(&mut cursor).write_ts_packet(&packet).unwrap();
        }
        loop {
            task::block_on(async {
                let (player_cid, total_size) = container.send_player_directory(&self.block_sender, &local_peer_id).await;
                log::warn!("ingest stream ended, final PLAYER ====> https://{}.{} ({} bytes)", player_cid.to_string(), IPFS_GATEWAY, total_size);
                self.pin_sender.send(player_cid).await;
                task::sleep(Duration::from_secs(60)).await;
            });
        }
    }
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

impl P2PVideoNode {
    fn new(block_receiver : Receiver<BlockInfo>) -> Result<P2PVideoNode, Box<dyn Error>> {

        let local_key = {
            // Using RSA keypair because the new elliptic curve based identity seems to break p2p-circuit routing
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
            identity::Keypair::rsa_from_pkcs8(&mut data)?
        };
        let local_peer_id = PeerId::from(local_key.public());
        log::info!("local identity is {}", local_peer_id.to_string());

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
            blocks_to_send: BTreeSet::new(),
            block_receiver,
        };

        behaviour.add_router_address(&IPFS_ROUTER_ID.parse().unwrap(), IPFS_ROUTER_ADDR_TCP.parse().unwrap());
        behaviour.add_router_address(&IPFS_ROUTER_ID.parse().unwrap(), IPFS_ROUTER_ADDR_UDP.parse().unwrap());
        behaviour.kad_wan.bootstrap().unwrap();
        behaviour.gossipsub.subscribe(gossipsub_topic.clone());

        let mut swarm = Swarm::new(transport, behaviour, local_peer_id.clone());
        Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(P2PVideoNode {
            gossipsub_topic,
            swarm
        })
    }

    fn store_block(&mut self, block_info: BlockInfo) {
        let usage = &block_info.usage;
        let block_size = block_info.block.data.len();
        let cid_str = block_info.block.cid.to_string();
        let cid_bytes = block_info.block.cid.to_bytes();
        let hash_bytes = block_info.block.cid.hash().to_bytes();
        let topic = self.gossipsub_topic.clone();

        match self.swarm.gossipsub.publish(&topic, cid_bytes.clone()) {
            Ok(()) => {},
            Err(PublishError::InsufficientPeers) => {},
            Err(err) => log::warn!("couldn't publish, {:?}", err)
        }

        let netinfo = Swarm::network_info(&mut self.swarm);
        log::info!("ingest {} bytes, cid {} {:?} ({} peers)", block_size, cid_str, usage, netinfo.num_peers);

        self.swarm.kad_lan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
        self.swarm.kad_wan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
        self.swarm.block_store.insert(hash_bytes, block_info);
    }
}

impl Future for P2PVideoNode {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        loop {
            let mut event_pending_counter = 3;

            match self.swarm.blocks_to_send.iter().cloned().next() {
                None => event_pending_counter -= 1,
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
            };

            match Pin::new(&mut self.swarm.block_receiver).poll_next(ctx) {
                Poll::Pending => event_pending_counter -= 1,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(block_info)) => self.store_block(block_info)
            }

            let network_event = unsafe { Pin::new_unchecked(&mut self.swarm.next_event()) }.poll(ctx);
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
    env_logger::from_env(Env::default().default_filter_or("rectangle_device=info")).init();
    let video_args = std::env::args().skip(1).collect();

    let (block_sender, block_receiver) = channel(32);
    let (pin_sender, pin_receiver) = channel(32);

    let node = P2PVideoNode::new(block_receiver)?;
    let local_peer_id = Swarm::local_peer_id(&node.swarm).clone();

    let pinner = Pinner {
        pin_receiver,
        local_multiaddrs: vec![
            format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
                router_addr = IPFS_ROUTER_ADDR_TCP, router_id = IPFS_ROUTER_ID, local_id = local_peer_id),
            format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
                router_addr = IPFS_ROUTER_ADDR_UDP, router_id = IPFS_ROUTER_ID, local_id = local_peer_id),
            format!("{router_addr:}/p2p/{router_id:}/p2p-circuit/p2p/{local_id:}",
                router_addr = IPFS_ROUTER_ADDR_WSS, router_id = IPFS_ROUTER_ID, local_id = local_peer_id)
        ]
    };

    thread::Builder::new().name("pinner".to_string()).spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(pinner.task());
    })?;

    thread::Builder::new().name("vid-in".to_string()).spawn(move || {
        VideoIngest {
            block_sender,
            pin_sender
        }.run(video_args, &local_peer_id)
    })?;

    task::Builder::new().name("p2p-node".to_string()).blocking(node);

    Ok(())
}
