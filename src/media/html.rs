// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use crate::media::hls::HLSContainer;
use libipld::cid::Cid;
use libp2p::PeerId;
use async_std::sync::Sender;
use rectangle_device_blocks::{BlockInfo, BlockUsage, PbLink};
use rectangle_device_blocks::raw::{RawBlockFile, MultiRawBlockFile};
use rectangle_device_blocks::dir::DirectoryBlock;
use rectangle_device_blocks::package::Package;
use rectangle_device_player::{IndexTemplate, Template, main_js};

fn index_html_template(hls_cid: &Cid, script_cid: &Cid, local_peer_id: &PeerId) -> String {
    let router_multiaddr = format!("{}/p2p/{}", config::IPFS_ROUTER_ADDR_WSS, config::IPFS_ROUTER_ID);
    let bootstrap = format!("{} {}/p2p-circuit/p2p/{}", router_multiaddr, router_multiaddr, local_peer_id.to_string());
    IndexTemplate {
        main_js_cid: &script_cid.to_string(),
        ipfs_gateway: config::IPFS_GATEWAY,
        ipfs_delegates: &router_multiaddr,
        ipfs_bootstrap: &bootstrap,
        hls_cid: &hls_cid.to_string(),
        hls_name: config::HLS_FILENAME,
    }.render().unwrap()
}

pub struct HLSPlayerDist {
    pub script: MultiRawBlockFile,
    pub links: Vec<PbLink>
}

impl HLSPlayerDist {
    pub fn new() -> HLSPlayerDist {
        let script = MultiRawBlockFile::from_bytes(main_js().as_bytes());
        let links = vec![
            script.link(config::JS_FILENAME.to_string()),
        ];
        HLSPlayerDist { script, links }
    }

    pub async fn send_copy(&self, sender: &Sender<BlockInfo>) {
        self.script.clone().send(sender, &BlockUsage::PlayerScript).await;
    }
}

pub struct HLSPlayer {
    pub html: RawBlockFile,
    pub directory: DirectoryBlock,
    pub sequence: usize
}

impl HLSPlayer {
    pub fn from_hls(hls: &HLSContainer, dist: &HLSPlayerDist, local_peer_id: &PeerId) -> HLSPlayer {
        HLSPlayer::from_link(
            hls.directory.link(config::HLS_DIRECTORY.to_string()),
            &dist.script.root.cid,
            &dist.links,
            hls.sequence,
            local_peer_id)
    }

    pub fn from_link(hls_link: PbLink, script_cid: &Cid, added_links: &Vec<PbLink>,
        sequence: usize, local_peer_id: &PeerId) -> HLSPlayer {

        let html_string = index_html_template(&hls_link.cid, script_cid, local_peer_id);
        let html = RawBlockFile::new(&html_string.as_bytes());
        let mut links = vec![
            html.link("index.html".to_string()),
            hls_link
        ];

        for link in added_links {
            links.push(PbLink {
                cid: link.cid.clone(),
                name: link.name.clone(),
                size: link.size
            });
        }

        HLSPlayer {
            html,
            sequence,
            directory: DirectoryBlock::new(links)
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>) {
        self.html.send(sender, &BlockUsage::Player(self.sequence)).await;
        self.directory.send(sender, &BlockUsage::PlayerDirectory(self.sequence)).await;
    }
}
