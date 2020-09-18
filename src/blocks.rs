use libipld::cid::Cid;
use libipld::Ipld;
use libipld::raw::RawCodec;
use libipld::codec_impl::Multicodec;
use libipld::pb::DagPbCodec;
use libipld::multihash::{Multihash, SHA2_256};
use std::collections::BTreeMap;

pub type Block = libipld::block::Block<Multicodec, Multihash>;

pub struct BlockInfo {
    pub block: Block,
    pub usage: BlockUsage,
}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq, Clone)]
pub enum BlockUsage {
    // We try to send blocks in the same order listed here
    PlayerDirectory(usize),
    Player(usize),
    VideoDirectory(usize),
    Playlist(usize),
    VideoSegment(usize),
}

pub fn make_pb_link(cid: Cid, size: usize, name: String) -> Ipld {
    let mut pb_link = BTreeMap::<String, Ipld>::new();
    pb_link.insert("Hash".to_string(), cid.clone().into());
    pb_link.insert("Name".to_string(), name.into());
    pb_link.insert("Tsize".to_string(), size.into());
    pb_link.into()
}

fn make_pb_node(links: Vec<Ipld>, data: Vec<u8>) -> Ipld {
    let mut pb_node = BTreeMap::<String, Ipld>::new();
    pb_node.insert("Links".to_string(), links.into());
    pb_node.insert("Data".to_string(), data.into());
    pb_node.into()
}

pub fn make_raw_block(data: &[u8]) -> Block {
    Block::encode(RawCodec, SHA2_256, data).unwrap()
}

fn make_unixfs_directory(links: Vec<Ipld>) -> Ipld {
    const PBTAG_TYPE: u8 = 8;
    const TYPE_DIRECTORY: u8 = 1;
    make_pb_node(links, vec![PBTAG_TYPE, TYPE_DIRECTORY])
}

pub fn make_directory_block(links: Vec<Ipld>) -> Block {
    let ipld = make_unixfs_directory(links);
    Block::encode(DagPbCodec, SHA2_256, &ipld).unwrap()
}
