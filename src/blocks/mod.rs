// This code may not be used for any purpose. Be gay, do crime.

mod dag;
mod unixfs;

pub use dag::Link;

use libipld::cid::Cid;
use libipld::Ipld;
use libipld::raw::RawCodec;
use libipld::codec_impl::Multicodec;
use libipld::pb::DagPbCodec;
use libipld::multihash::{Multihash, SHA2_256};
use std::collections::BTreeMap;

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq, Clone)]
pub enum BlockUsage {
    // We try to send blocks in the same order listed here
    PlayerDirectory(usize),
    Player(usize),
    VideoDirectory(usize),
    Playlist(usize),
    VideoSegment(usize),
}

pub type Block = libipld::block::Block<Multicodec, Multihash>;

pub struct BlockInfo {
    pub block: Block,
    pub usage: BlockUsage,
}

#[derive(Clone)]
pub struct Link {
    pub cid: Cid,
    pub name: String,
    pub size: usize,
}

pub struct DirectoryBlock {
    pub block: Block,
    pub total_size: usize,
    pub links: Vec<Link>
}

impl DirectoryBlock {
    pub fn new(links: Vec<Link>) -> DirectoryBlock {
        let mut total_size = 0;
        let mut ipld = vec![];
        for link in links.clone() {
            total_size += link.size;
            ipld.push(make_pb_link(link));
        }
        let ipld = make_unixfs_directory(links);
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

    pub fn use_as(self, usage: BlockUsage) -> BlockInfo {
        BlockInfo {
            block: self.block,
            usage
        }
    }
}

pub struct RawFileBlock {
    pub block: Block,
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

    pub fn use_as(self, usage: BlockUsage) -> BlockInfo {
        BlockInfo {
            block: self.block,
            usage
        }
    }
}
