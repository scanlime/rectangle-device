// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use crate::media::MediaContainer;
use rectangle_device_blocks::{BlockInfo, BlockUsage, PbLink};
use rectangle_device_blocks::raw::RawBlockFile;
use rectangle_device_blocks::dir::DirectoryBlock;
use rectangle_device_blocks::package::Package;
use m3u8_rs::playlist::{MediaPlaylist, MediaSegment, MediaPlaylistType};
use async_std::sync::Sender;

pub struct HLSContainer {
    pub playlist: RawBlockFile,
    pub directory: DirectoryBlock,
    pub sequence: usize
}

impl HLSContainer {
    pub fn new(mc: &MediaContainer) -> HLSContainer {
        let mut hls_segments = vec![];
        let mut links = vec![];

        if let Some(highest_seq) = mc.blocks.iter().map(|segment| segment.sequence).max() {
            let seq_digits = highest_seq.to_string().len();

            for segment in &mc.blocks {
                let filename = format!("z{0:01$}.ts", segment.sequence, seq_digits);

                links.push(PbLink {
                    cid: segment.cid.clone(),
                    name: filename.clone(),
                    size: segment.bytes as u64
                });
                hls_segments.push(MediaSegment {
                    uri: filename.into(),
                    duration: segment.duration,
                    ..Default::default()
                });
            }
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
        let playlist = RawBlockFile::new(&playlist_data);

        // Put the HLS playlist link first in the directory
        links.insert(0, playlist.link(config::HLS_FILENAME.to_string()));

        HLSContainer {
            playlist: playlist,
            directory: DirectoryBlock::new(links),
            sequence: hls_playlist.segments.len()
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>) {
        self.playlist.send(sender, &BlockUsage::Playlist(self.sequence)).await;
        self.directory.send(sender, &BlockUsage::VideoDirectory(self.sequence)).await;
    }
}
