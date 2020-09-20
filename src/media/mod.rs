// This code may not be used for any purpose. Be gay, do crime.

pub mod hls;
pub mod html;

use libipld::cid::Cid;

pub struct MediaBlockInfo {
    pub cid: Cid,
    pub bytes: usize,
    pub duration: f32,
    pub sequence: usize
}

pub struct MediaContainer {
    pub blocks: Vec<MediaBlockInfo>,
}
