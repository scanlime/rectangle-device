pub mod hls;
pub mod html;
pub mod ingest;

use async_std::sync::{channel, Receiver, Sender};
use rectangle_device_blocks::{BlockInfo, Cid};

#[derive(Clone)]
pub struct MediaBlockInfo {
    pub cid: Cid,
    pub bytes: usize,
    pub duration: f32,
    pub sequence: usize,
}

#[derive(Clone)]
pub struct MediaContainer {
    pub blocks: Vec<MediaBlockInfo>,
}

impl MediaContainer {
    pub fn new() -> MediaContainer {
        MediaContainer { blocks: vec![] }
    }
}

pub enum MediaUpdate {
    Block(BlockInfo),
    Container(MediaContainer),
}

#[derive(Clone)]
pub struct MediaUpdateBus {
    pub sender: Sender<MediaUpdate>,
    pub receiver: Receiver<MediaUpdate>,
}

impl MediaUpdateBus {
    pub fn new() -> MediaUpdateBus {
        let (sender, receiver) = channel(32);
        MediaUpdateBus { sender, receiver }
    }
}
