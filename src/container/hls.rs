// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use crate::container::media::Container;
use crate::blocks::{BlockInfo, BlockUsage, DirectoryBlock, Link, RawFileBlock};
use m3u8_rs::playlist::{MediaPlaylist, MediaSegment, MediaPlaylistType};
use async_std::sync::Sender;

pub struct HLSContainer {
    pub playlist: RawFileBlock,
    pub directory: DirectoryBlock,
    pub sequence: usize
}

impl HLSContainer {
    pub fn new(container: &Container) -> HLSContainer {
        let mut hls_segments = vec![];
        let mut links = vec![];

        for segment in &container.blocks {
            let filename = format!("s{:05}.ts", segment.sequence);

            links.push(Link {
                cid: segment.cid.clone(),
                name: filename.clone(),
                size: segment.bytes
            });
            hls_segments.push(MediaSegment {
                uri: filename.into(),
                duration: segment.duration,
                ..Default::default()
            });
        }

        // https://tools.ietf.org/html/rfc8216
        let hls_playlist = MediaPlaylist {
            version: 3,
            target_duration: config::SEGMENT_MAX_SEC,
            // To Do: Want to set this to true but it would be a lie until we can split h264 too
            independent_segments: false,
            media_sequence: 0,
            playlist_type: Some(MediaPlaylistType::Vod),
            end_list: true,
            segments: hls_segments,
            ..Default::default()
        };

        let mut playlist_data: Vec<u8> = Vec::new();
        hls_playlist.write_to(&mut playlist_data).unwrap();
        let playlist = RawFileBlock::new(&playlist_data);

        // Put the HLS playlist link first in the directory
        links.insert(0, playlist.link(config::HLS_FILENAME.to_string()));

        HLSContainer {
            playlist: playlist,
            directory: DirectoryBlock::new(links),
            sequence: hls_playlist.segments.len()
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>) {
        sender.send(self.playlist.use_as(BlockUsage::Playlist(self.sequence))).await;
        sender.send(self.directory.use_as(BlockUsage::VideoDirectory(self.sequence))).await;
    }
}
