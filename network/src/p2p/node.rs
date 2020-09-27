// This code may not be used for any purpose. Be gay, do crime.

use crate::warmer::Warmer;
use crate::pinner::Pinner;
use crate::keypair::keypair_from_openssl_rsa;
use crate::p2p::behaviour::P2PVideoBehaviour;
use crate::p2p::config::P2PConfig;
use crate::p2p::peers::PeerAddr;
use rectangle_device_media::{MediaContainer, MediaUpdate, MediaUpdateBus};
use rectangle_device_media::hls::HLSContainer;
use rectangle_device_media::html::{HLSPlayer, HLSPlayerDist, PlayerNetworkConfig};
use rectangle_device_blocks::{BlockUsage, BlockInfo};
use rectangle_device_blocks::package::Package;
use async_std::sync::Receiver;
use core::pin::Pin;
use futures::{Future, Stream};
use libipld::cid::Cid;
use async_std::task::{self, Poll, Context};
use libp2p::{PeerId, Swarm, Multiaddr};
use libp2p::core::multiaddr::Protocol;
use libp2p::gossipsub::error::PublishError;
use libp2p::kad;
use libp2p::swarm::{SwarmEvent, NetworkBehaviour};
use rand::thread_rng;
use rand::seq::SliceRandom;
use std::thread;
use std::error::Error;

pub struct P2PVideoNode {
    config: P2PConfig,
    warmer: Warmer,
    pinner: Pinner,
    swarm: Swarm<P2PVideoBehaviour>,
    media_receiver: Option<Receiver<MediaUpdate>>,
    hls_player_dist: HLSPlayerDist,
}

impl P2PVideoNode {
    pub fn new(mub: &MediaUpdateBus, config: P2PConfig) -> Result<P2PVideoNode, Box<dyn Error>> {

        let pinner = Pinner::new();
        let warmer = Warmer::new();

        let local_key = keypair_from_openssl_rsa()?;
        let local_peer_id = PeerId::from(local_key.public());
        log::info!("local identity is {}", local_peer_id.to_string());

        let transport = libp2p::build_development_transport(local_key.clone())?;
        let behaviour = P2PVideoBehaviour::new(local_key, &local_peer_id, &config)?;
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id.clone());

        let bootstrap: Vec<PeerAddr> = swarm.configured_peers.node_bootstrap().into_iter().collect();
        let circuit_addrs: Vec<Multiaddr> = swarm.configured_peers.node_circuit_addrs().into_iter().collect();

        // All configured peers get used to bootstrap our DHT
        for peer in bootstrap {
            swarm.kad_wan.add_address(&peer.peer_id, peer.addr);
        }

        // Add "listen" addresses using p2p-circuit via our configured routers
        for mut addr in circuit_addrs {
            swarm.inject_new_listen_addr(&addr);
            addr.push(Protocol::P2p(local_peer_id.clone().into()));
            log::info!("listening via router {}", addr);
        }

        swarm.kad_wan.bootstrap().unwrap();

        let topic = swarm.gossipsub_topic.clone();
        swarm.gossipsub.subscribe(topic);

        // Regular local listening addresses, if we have any
        for addr in &config.listen_addrs {
            Swarm::listen_on(&mut swarm, addr.clone())?;
        }

        let mut node = P2PVideoNode {
            hls_player_dist: HLSPlayerDist::new(),
            media_receiver: Some(mub.receiver.clone()),
            config,
            warmer,
            pinner,
            swarm,
        };

        // Store the player distribution blocks
        for block_info in node.hls_player_dist.copy_blocks() {
            node.store_block(block_info);
        }

        Ok(node)
    }

    pub fn run_blocking(self) -> Result<(), Box<dyn Error>> {
        let warmer = self.warmer.clone();
        let pinner = self.pinner.clone();

        let warmer_thread = thread::Builder::new().name("net-warmer".to_string())
            .spawn(move || tokio::runtime::Runtime::new().unwrap().block_on(warmer.task()))?;

        let pinner_thread = thread::Builder::new().name("net-pinner".to_string())
            .spawn(move || tokio::runtime::Runtime::new().unwrap().block_on(pinner.task()))?;

        task::Builder::new().name("net-node".to_string()).blocking(self);

        warmer_thread.join().unwrap();
        pinner_thread.join().unwrap();
        Ok(())
    }

    fn media_update_received(&mut self, update: MediaUpdate) {
        match update {
            MediaUpdate::Block(block_info) => self.store_block(block_info),
            MediaUpdate::Container(mc) => self.publish_media_container(mc),
        }
    }

    fn publish_media_container(&mut self, mc: MediaContainer) {
        if let Some(player_net) = self.configure_player() {
            let player_dist = &self.hls_player_dist;
            let hls = HLSContainer::new(&mc);
            let player = HLSPlayer::from_hls(&hls, player_dist, &player_net);
            let player_cid = &player.directory.block.cid;

            log::info!("PLAYER --- https://{}.ipfs.{}",
                player_cid.to_string(),
                player_net.gateway);

            log::info!("total size {} bytes, {:?}",
                player.directory.total_size(),
                player_net);

            for block_info in hls.into_blocks().into_iter().chain(player.into_blocks()) {
                self.store_block(block_info);
            }
        }
    }

    fn store_block(&mut self, block_info: BlockInfo) {
        let cid = block_info.block.cid.clone();
        let usage = block_info.usage.clone();

        let topic = self.swarm.gossipsub_topic.clone();
        match self.swarm.gossipsub.publish(&topic, cid.to_bytes()) {
            Ok(()) => {},
            Err(PublishError::InsufficientPeers) => {},
            Err(err) => log::warn!("couldn't publish, {:?}", err)
        }

        log::debug!("{:?}", Swarm::network_info(&mut self.swarm));
        log::debug!("stored {:7} bytes, {} {:?}",
            block_info.block.data.len(), block_info.block.cid.to_string(), usage);

        let hash_bytes = block_info.block.cid.hash().to_bytes();
        self.swarm.kad_lan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();
        self.swarm.kad_wan.start_providing(kad::record::Key::new(&hash_bytes)).unwrap();

        self.swarm.block_store.data.insert(hash_bytes, block_info);

        self.warmup_block(&cid, &usage);
        self.pin_block(&cid, &usage);
    }

    fn warmup_block(&self, cid: &Cid, usage: &BlockUsage) {
        let cid_str = cid.to_string();

        // Load this block into all gateways that we expect to pin this block eventually
        for gateway in &self.config.pinning_gateways {
            self.warmer.send(gateway.join("ipfs/").unwrap().join(&cid_str).unwrap());
        }

        match usage {
            BlockUsage::VideoSegment(_) => (),
            _ => {
                for gateway in &self.config.public_gateways {
                    // Only send non-video (player, directory) blocks to public gateways
                    self.warmer.send(gateway.join("ipfs/").unwrap().join(&cid_str).unwrap());
                }
            }
        }
    }

    fn pin_block(&self, cid: &Cid, usage: &BlockUsage) {
        match usage {
            BlockUsage::PlayerDirectory(_) => {
                // Only pin the top-most object; player directories link to everything else
                for api in &self.config.pinning_services {
                    let peer_id = Swarm::local_peer_id(&self.swarm);
                    let addrs = Swarm::external_addresses(&self.swarm);
                    let origins: Vec<String> = addrs.map(|addr| format!("{}/p2p/{}", addr, peer_id)).collect();
                    self.pinner.send(api.clone(), cid.to_string(), format!("{:?}", usage), origins);
                }
            },
            _ => {}
        }
    }

    fn configure_player(&self) -> Option<PlayerNetworkConfig> {
        // Choose a random https gateway, and keep only the hostname part
        let gateway = {
            let mut gateways = vec![];
            for url in &self.config.public_gateways {
                if url.scheme() == "https" && url.port_or_known_default() == Some(443) {
                    if let Some(s) = url.host_str() {
                        gateways.push(s.to_string());
                    }
                }
            }
            let mut rng = thread_rng();
            match gateways.choose_mut(&mut rng) {
                Some(g) => Some(std::mem::take(g)),
                None => None
            }
        };

        let delegates: Vec<String> = self.swarm.configured_peers
            .player_delegates().into_iter().map(|addr| addr.to_string()).collect();
        let bootstrap: Vec<String> = self.swarm.configured_peers
            .player_bootstrap().into_iter().map(|addr| addr.to_string()).collect();

        if delegates.is_empty() {
            log::error!("can't configure the player without a viable delegate node");
            None
        } else {
            match gateway {
                None => {
                    log::error!("can't configure the player without at least one public gateway");
                    None
                },
                Some(gateway) => Some(PlayerNetworkConfig {
                    gateway, delegates, bootstrap,
                })
            }
        }
    }
}

impl Future for P2PVideoNode {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {

        // At most one bitswap sent block per wakeup for now, to keep networ from getting overwhelmed
        match self.swarm.block_store.next_send() {
            None => {},
            Some(request) => {
                if let Some(block_info) = self.swarm.block_store.data.get(&request.cid.hash().to_bytes()) {
                    log::debug!("SENDING block in response to want, {} {:?} -> {}",
                        request.cid.to_string(), request.usage, request.peer_id);
                    let data = block_info.block.data.clone();
                    self.swarm.bitswap.send_block(&request.peer_id, request.cid, data);
                }
            }
        }

        // Fully drain the media receiver
        loop {
            match self.media_receiver.take() {
                None => {
                    break;
                },
                Some(mut receiver) => {
                    let event = Pin::new(&mut receiver).poll_next(ctx);
                    match event {
                        Poll::Pending => {
                            self.media_receiver = Some(receiver);
                            break;
                        },
                        Poll::Ready(None) => {
                            drop(receiver);
                            break;
                        },
                        Poll::Ready(Some(update)) => {
                            self.media_receiver = Some(receiver);
                            self.media_update_received(update);
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
