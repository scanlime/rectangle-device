// This code may not be used for any purpose. Be gay, do crime.

mod dag;
mod unixfs;

pub use dag::Link;

use libipld::raw::RawCodec;
use libipld::codec_impl::Multicodec;
use libipld::pb::DagPbCodec;
use libipld::multihash::{Multihash, SHA2_256};
use async_std::sync::Sender;
use crate::config::SEGMENT_MAX_BYTES;

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq, Clone)]
pub enum BlockUsage {
    // We try to send blocks in the same order listed here
    PlayerDirectory(usize),
    PlayerScript,
    Player(usize),
    VideoDirectory(usize),
    Playlist(usize),
    VideoSegment(usize),
}

pub type Block = libipld::block::Block<Multicodec, Multihash>;

#[derive(Clone)]
pub struct BlockInfo {
    pub block: Block,
    pub usage: BlockUsage,
}

#[derive(Clone)]
pub struct MultiBlockFile {
    pub root: Block,
    pub total_size: usize,
    pub parts: Vec<RawFileBlock>,
}

#[derive(Clone)]
pub struct RawFileBlock {
    pub block: Block,
}

#[derive(Clone)]
pub struct DirectoryBlock {
    pub block: Block,
    pub total_size: usize,
    pub links: Vec<Link>
}

impl MultiBlockFile {
    pub fn new(bytes: &[u8]) -> MultiBlockFile {
        let mut total_size = 0;
        let mut parts = vec![];
        let mut sizes = vec![];
        let mut ipld = vec![];

        for chunk in bytes.chunks(SEGMENT_MAX_BYTES) {
            let part = RawFileBlock::new(chunk);
            let link = part.link("".to_string());
            total_size += link.size;
            sizes.push(link.size);
            ipld.push(dag::make_pb_link(link));
            parts.push(part);
        }

        let ipld = unixfs::make_file(ipld, sizes);
        let root = Block::encode(DagPbCodec, SHA2_256, &ipld).unwrap();
        total_size += root.data.len();

        MultiBlockFile { root, total_size, parts }
    }

    pub fn link(&self, name: String) -> Link {
        Link {
            cid: self.root.cid.clone(),
            size: self.total_size,
            name,
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>, usage: BlockUsage) {
        sender.send(BlockInfo {
            block: self.root,
            usage: usage.clone()
        }).await;
        for part in self.parts {
            sender.send(BlockInfo {
                block: part.block,
                usage: usage.clone()
            }).await;
        }
    }
}

impl DirectoryBlock {
    pub fn new(links: Vec<Link>) -> DirectoryBlock {
        let mut total_size = 0;
        let mut ipld = vec![];
        for link in links.clone() {
            total_size += link.size;
            ipld.push(dag::make_pb_link(link));
        }
        let ipld = unixfs::make_directory(ipld);
        let block = Block::encode(DagPbCodec, SHA2_256, &ipld).unwrap();
        total_size += block.data.len();
        DirectoryBlock { block, total_size, links }
    }

    pub fn link(&self, name: String) -> Link {
        Link {
            cid: self.block.cid.clone(),
            size: self.total_size,
            name,
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>, usage: BlockUsage) {
        sender.send(BlockInfo { block: self.block, usage }).await;
    }
}

impl RawFileBlock {
    pub fn new(data: &[u8]) -> RawFileBlock {
        RawFileBlock {
            block: Block::encode(RawCodec, SHA2_256, data).unwrap()
        }
    }

    pub fn link(&self, name: String) -> Link {
        Link {
            cid: self.block.cid.clone(),
            size: self.block.data.len(),
            name,
        }
    }

    pub async fn send(self, sender: &Sender<BlockInfo>, usage: BlockUsage) {
        sender.send(BlockInfo { block: self.block, usage }).await;
    }
}
