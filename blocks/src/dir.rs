use crate::{
    core::{Block, Cid, DefaultHashType, BLOCK_MAX_BYTES},
    package::Package,
    unixfs,
};
use libipld::{
    pb::{DagPbCodec, PbNode},
    Ipld,
};
use prost::Message;
use std::{
    convert::TryFrom,
    iter::{once, Once},
};

pub use libipld::pb::PbLink;

pub struct DirectoryBlock {
    pub block: Block,
    pub links: Vec<PbLink>,
}

impl DirectoryBlock {
    pub fn new(links: Vec<PbLink>) -> DirectoryBlock {
        let dir = unixfs::pb::Data {
            r#type: unixfs::pb::data::DataType::Directory.into(),
            blocksizes: vec![],
            filesize: None,
            data: None,
            hash_type: None,
            fanout: None,
        };

        let mut data = vec![];
        dir.encode(&mut data).unwrap();
        let node = PbNode {
            data: data.into_boxed_slice(),
            links,
        };
        let ipld: Ipld = node.into();

        // This is a single block directory. It can only hold ~10000-20000 files
        let block = Block::encode(DagPbCodec, DefaultHashType, &ipld).unwrap();
        assert!(block.data.len() <= BLOCK_MAX_BYTES);

        let links = PbNode::try_from(&ipld).unwrap().links;
        DirectoryBlock { block, links }
    }
}

impl Package for DirectoryBlock {
    type BlockIterator = Once<Block>;

    fn cid(&self) -> &Cid {
        &self.block.cid
    }

    fn total_size(&self) -> u64 {
        let linked_size: u64 = self.links.iter().map(|link| link.size).sum();
        let dir_block_size = self.block.data.len() as u64;
        linked_size + dir_block_size
    }

    fn into_blocks(self) -> Self::BlockIterator {
        once(self.block)
    }
}
