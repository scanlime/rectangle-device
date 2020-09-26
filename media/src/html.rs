// This code may not be used for any purpose. Be gay, do crime.

use crate::hls::{HLSContainer, HLS_FILENAME};
use rectangle_device_blocks::{Cid, BlockUsage, BlockInfo, PbLink};
use rectangle_device_blocks::raw::{RawBlockFile, MultiRawBlockFile};
use rectangle_device_blocks::dir::DirectoryBlock;
use rectangle_device_blocks::package::Package;
use rectangle_device_player::{IndexTemplate, Template, main_js};

pub const JS_FILENAME : &'static str = "main.js";
pub const HTML_FILENAME : &'static str = "index.html";
pub const HLS_DIRECTORY : &'static str = "video";

pub struct PlayerNetworkConfig {
    pub gateway: String,
    pub delegates: Vec<String>,
    pub bootstrap: Vec<String>,
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

    pub fn copy_blocks(&self) -> impl IntoIterator<Item = BlockInfo> {
        let usage = BlockUsage::PlayerScript;
        self.script.clone().into_blocks().map(move |block| usage.attach_to(block))
    }
}

impl HLSPlayer {
    pub fn from_hls(hls: &HLSContainer, dist: &HLSPlayerDist, network: &PlayerNetworkConfig) -> HLSPlayer {
        HLSPlayer::from_link(
            hls.directory.link(HLS_DIRECTORY.to_string()),
            &dist.script.root.cid,
            &dist.links,
            hls.sequence,
            network)
    }

    pub fn from_link(hls_link: PbLink, script_cid: &Cid, added_links: &Vec<PbLink>,
        sequence: usize, network: &PlayerNetworkConfig) -> HLSPlayer {

        let html_string = IndexTemplate {
            gateway: &network.gateway,
            delegates: &network.delegates.join(" "),
            bootstrap: &network.bootstrap.join(" "),
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

    pub fn into_blocks(self) -> impl IntoIterator<Item = BlockInfo> {
        let player_usage = BlockUsage::Player(self.sequence);
        let directory_usage = BlockUsage::PlayerDirectory(self.sequence);

        self.html.into_blocks().map(
            move |block| player_usage.attach_to(block)
        ).chain(self.directory.into_blocks().map(
            move |block| directory_usage.attach_to(block)
        ))
    }
}
