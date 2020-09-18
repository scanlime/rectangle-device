use crate::config;
use crate::blocks::{BlockInfo, BlockUsage, make_pb_link, make_raw_block, make_directory_block};
use m3u8_rs::playlist::{MediaPlaylist, MediaSegment, MediaPlaylistType};
use libipld::Ipld;
use libipld::cid::Cid;
use libp2p::PeerId;
use async_std::sync::Sender;

pub struct VideoContainer {
    pub blocks: Vec<(Cid, usize, f32)>,
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
            target_duration: config::SEGMENT_MAX_SEC,
            // To Do: Want to set this to true but it would be a lie until we can split h264 too
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
        links.push(make_pb_link(block.cid.clone(), block.data.len(), config::HLS_FILENAME.to_string()));

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
            player = config::IPFS_PLAYER_CID,
            gateway = config::IPFS_GATEWAY,
            router_addr = config::IPFS_ROUTER_ADDR_WSS,
            router_id = config::IPFS_ROUTER_ID,
            hls_cid = hls_cid.to_string(),
            hls_name = config::HLS_FILENAME,
            local_id = local_peer_id.to_string());

        let block = make_raw_block(&html_data.as_bytes());
        links.push(make_pb_link(
            block.cid.clone(),
            block.data.len(),
            "index.html".to_string()
        ));
        links.push(make_pb_link(
            config::IPFS_PLAYER_CID.parse().unwrap(),
            config::IPFS_PLAYER_SIZE,
            config::IPFS_PLAYER_NAME.to_string()
        ));
        links.push(make_pb_link(
            hls_cid.clone(),
            hls_size,
            config::HLS_DIRECTORY.to_string()
        ));
        *total_size += block.data.len() + config::IPFS_PLAYER_SIZE + hls_size;

        BlockInfo {
            block: block,
            usage: BlockUsage::Player(self.blocks.len()),
        }
    }

    pub async fn send_hls_directory(&self, block_sender: &Sender<BlockInfo>) -> (Cid, usize) {
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

    pub async fn send_player_directory(&self, block_sender: &Sender<BlockInfo>, local_peer_id: &PeerId) -> (Cid, usize) {
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
