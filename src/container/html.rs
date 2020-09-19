// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use crate::blocks::{BlockInfo, BlockUsage, DirectoryBlock, Link, RawFileBlock, MultiBlockFile};
use crate::container::hls::HLSContainer;
use libipld::cid::Cid;
use libp2p::PeerId;
use async_std::sync::Sender;

fn index_html_template(hls_cid: &Cid, script_cid: &Cid, local_peer_id: &PeerId) -> String {
    format!(r#"<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=100.0, minimum-scale=1.0" />
        <link rel="icon" href="data:," />
        <script src="https://{script:}.ipfs.{gateway:}/"></script>
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
    script = script_cid.to_string(),
    gateway = config::IPFS_GATEWAY,
    router_addr = config::IPFS_ROUTER_ADDR_WSS,
    router_id = config::IPFS_ROUTER_ID,
    hls_cid = hls_cid.to_string(),
    hls_name = config::HLS_FILENAME,
    local_id = local_peer_id.to_string())
}

#[derive(Clone)]
pub struct HLSPlayerDist {
    pub script: MultiBlockFile,
    pub links: Vec<Link>
}

impl HLSPlayerDist {
    pub fn new() -> HLSPlayerDist {
        let script = MultiBlockFile::new(include_bytes!("../../player/dist/bundle.js"));
        let script_link = script.link(config::JS_FILENAME.to_string());
        HLSPlayerDist {
            script,
            links: vec![
                script_link
            ]
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>) {
        self.script.send(sender, BlockUsage::PlayerScript).await;
    }
}

pub struct HLSPlayer {
    pub hls: Link,
    pub html: RawFileBlock,
    pub directory: DirectoryBlock,
    pub sequence: usize
}

impl HLSPlayer {
    pub fn from_hls(hls: &HLSContainer, dist: &HLSPlayerDist, local_peer_id: &PeerId) -> HLSPlayer {
        let hls_link = hls.directory.link(config::HLS_DIRECTORY.to_string());
        let mut added_links = hls.directory.links.clone();
        added_links.append(&mut dist.links.clone());
        let script_cid = &dist.script.root.cid;
        HLSPlayer::from_link(&hls_link, script_cid, &added_links, hls.sequence, local_peer_id)
    }

    pub fn from_link(hls_link: &Link, script_cid: &Cid, added_links: &Vec<Link>,
        sequence: usize, local_peer_id: &PeerId) -> HLSPlayer {

        let html_string = index_html_template(&hls_link.cid, script_cid, local_peer_id);
        let html = RawFileBlock::new(&html_string.as_bytes());
        let mut links = vec![
            html.link("index.html".to_string()),
            hls_link.clone(),
        ];
        links.append(&mut added_links.clone());
        HLSPlayer {
            hls: hls_link.clone(),
            html,
            sequence,
            directory: DirectoryBlock::new(links)
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>) {
        self.html.send(sender, BlockUsage::Player(self.sequence)).await;
        self.directory.send(sender, BlockUsage::PlayerDirectory(self.sequence)).await;
    }
}
