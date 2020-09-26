// This code may not be used for any purpose. Be gay, do crime.

use crate::hls::{HLSContainer, HLS_FILENAME};
use async_std::sync::Sender;
use rand::seq::SliceRandom;
use rectangle_device_blocks::{Cid, BlockInfo, BlockUsage, PbLink};
use rectangle_device_blocks::raw::{RawBlockFile, MultiRawBlockFile};
use rectangle_device_blocks::dir::DirectoryBlock;
use rectangle_device_blocks::package::Package;
use rectangle_device_player::{IndexTemplate, Template, main_js};

pub const JS_FILENAME : &'static str = "main.js";
pub const HTML_FILENAME : &'static str = "index.html";
pub const HLS_DIRECTORY : &'static str = "video";

pub struct PlayerNetworkConfig {
    pub ipfs_gateways: Vec<String>,
    pub ipfs_delegates: Vec<String>,
    pub ipfs_bootstrap: Vec<String>,
}

pub struct HLSPlayer {
    pub html: RawBlockFile,
    pub directory: DirectoryBlock,
    pub sequence: usize
}

pub struct HLSPlayerDist {
    pub script: MultiRawBlockFile,
    pub links: Vec<PbLink>
}

impl HLSPlayerDist {
    pub fn new() -> HLSPlayerDist {
        let script = MultiRawBlockFile::from_bytes(main_js().as_bytes());
        let links = vec![
            script.link(JS_FILENAME.to_string()),
        ];
        HLSPlayerDist { script, links }
    }

    pub async fn send_copy(&self, sender: &Sender<BlockInfo>) {
        self.script.clone().send(sender, &BlockUsage::PlayerScript).await;
    }
}

impl HLSPlayer {
    pub fn from_hls(hls: &HLSContainer, dist: &HLSPlayerDist, network: &PlayerNetworkConfig) -> HLSPlayer {
        assert!(network.ipfs_gateways.len() >= 1);
        HLSPlayer::from_link(
            hls.directory.link(HLS_DIRECTORY.to_string()),
            &dist.script.root.cid,
            &dist.links,
            hls.sequence,
            network)
    }

    pub fn from_link(hls_link: PbLink, script_cid: &Cid, added_links: &Vec<PbLink>,
        sequence: usize, network: &PlayerNetworkConfig) -> HLSPlayer {

        let mut rng = rand::thread_rng();
        let ipfs_gateway = network.ipfs_gateways.choose(&mut rng).unwrap();
        let ipfs_delegates = network.ipfs_delegates.join(" ");
        let ipfs_bootstrap = network.ipfs_bootstrap.join(" ");

        let html_string = IndexTemplate {
            ipfs_gateway: &ipfs_gateway,
            ipfs_delegates: &ipfs_delegates,
            ipfs_bootstrap: &ipfs_bootstrap,
            hls_cid: &hls_link.cid.to_string(),
            main_js_cid: &script_cid.to_string(),
            hls_name: HLS_FILENAME,
        }.render().unwrap();

        let html = RawBlockFile::new(&html_string.as_bytes());
        let mut links = vec![
            html.link(HTML_FILENAME.to_string()),
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
