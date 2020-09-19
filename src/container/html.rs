// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use crate::blocks::{BlockInfo, BlockUsage, DirectoryBlock, Link, RawFileBlock};
use crate::container::hls::HLSContainer;
use libipld::cid::Cid;
use libp2p::PeerId;
use async_std::sync::Sender;

fn index_html_template(hls_cid: &Cid, local_peer_id: &PeerId) -> String {
    format!(r#"<!DOCTYPE html>
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
    player = config::IPFS_PLAYER_CID,
    gateway = config::IPFS_GATEWAY,
    router_addr = config::IPFS_ROUTER_ADDR_WSS,
    router_id = config::IPFS_ROUTER_ID,
    hls_cid = hls_cid.to_string(),
    hls_name = config::HLS_FILENAME,
    local_id = local_peer_id.to_string())
}

pub struct HLSPlayer {
    pub hls: Link,
    pub html: RawFileBlock,
    pub directory: DirectoryBlock,
    pub sequence: usize
}

impl HLSPlayer {
    pub fn from_hls(hls: &HLSContainer, local_peer_id: &PeerId) -> HLSPlayer {
        let hls_link = hls.directory.link(config::HLS_DIRECTORY.to_string());
        let added_links = &hls.directory.links;
        HLSPlayer::from_link(&hls_link, added_links, hls.sequence, local_peer_id)
    }

    pub fn from_link(hls_link: &Link, added_links: &Vec<Link>, sequence: usize, local_peer_id: &PeerId) -> HLSPlayer {
        let html_string = index_html_template(&hls_link.cid, local_peer_id);
        let html = RawFileBlock::new(&html_string.as_bytes());
        let mut links = vec![
            html.link("index.html".to_string()),
            hls_link.clone(),
            Link {
                cid: config::IPFS_PLAYER_CID.parse().unwrap(),
                size: config::IPFS_PLAYER_SIZE,
                name: config::IPFS_PLAYER_NAME.to_string()
            },
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
        sender.send(self.html.use_as(BlockUsage::Player(self.sequence))).await;
        sender.send(self.directory.use_as(BlockUsage::PlayerDirectory(self.sequence))).await;
    }
}
