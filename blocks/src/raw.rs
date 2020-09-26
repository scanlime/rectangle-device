// This code may not be used for any purpose. Be gay, do crime.

use libipld::Ipld;
use libipld::raw::RawCodec;
use libipld::pb::{DagPbCodec, PbLink, PbNode};
use prost::Message;
use std::iter::{once, Once, Chain, Map, Flatten};
use std::vec;
use crate::core::{Block, Cid, DefaultHashType, BLOCK_MAX_BYTES};
use crate::package::Package;
use crate::unixfs;

#[derive(Clone)]
pub struct RawBlockFile {
    pub block: Block,
}

impl RawBlockFile {
    pub fn new(data: &[u8]) -> RawBlockFile {
        assert!(data.len() <= BLOCK_MAX_BYTES);
        RawBlockFile { block: Block::encode(RawCodec, DefaultHashType, data).unwrap() }
    }
}

impl Package for RawBlockFile {
    type BlockIterator = Once<Block>;

    fn cid(&self) -> &Cid {
        &self.block.cid
    }

    fn total_size(&self) -> u64 {
        self.block.data.len() as u64
    }

    fn into_blocks(self) -> Self::BlockIterator {
        once(self.block)
    }
}

#[derive(Clone)]
pub struct MultiRawBlockFile {
    pub root: Block,
    pub parts: Vec<RawBlockFile>,
}

impl MultiRawBlockFile {
    pub fn from_bytes(bytes: &[u8]) -> MultiRawBlockFile {
        let chunks = bytes.chunks(BLOCK_MAX_BYTES);
        let parts = chunks.map(|chunk| RawBlockFile::new(chunk));
        MultiRawBlockFile::new(parts.collect())
    }

    pub fn new(parts: Vec<RawBlockFile>) -> MultiRawBlockFile {
        let links: Vec<PbLink> = parts.iter().map(|part| part.link("".to_string())).collect();
        let sizes: Vec<u64> = parts.iter().map(|part| part.block.data.len() as u64).collect();
        let filesize = sizes.iter().sum();

        let file = unixfs::pb::Data {
            r#type: unixfs::pb::data::DataType::File.into(),
            blocksizes: sizes,
            filesize: Some(filesize),
            data: None,
            hash_type: None,
            fanout: None
        };

        let node = {
            let mut data: Vec<u8> = vec![];
            file.encode(&mut data).unwrap();
            PbNode { links, data: data.into_boxed_slice() }
        };
        let ipld: Ipld = node.into();

        // It takes a file size of about 32GB to reach this limit.
        // If it's ever realistic for us to hit this limit, we could use additional index blocks.
        let root = Block::encode(DagPbCodec, DefaultHashType, &ipld).unwrap();
        assert!(root.data.len() <= BLOCK_MAX_BYTES);

        MultiRawBlockFile { root, parts }
    }
}

type RawIntoBlocksFn = fn(RawBlockFile) -> <RawBlockFile as Package>::BlockIterator;

impl Package for MultiRawBlockFile {
    type BlockIterator = Chain<Once<Block>, Flatten<Map< vec::IntoIter<RawBlockFile>, RawIntoBlocksFn >>>;

    fn cid(&self) -> &Cid {
        &self.root.cid
    }

    fn total_size(&self) -> u64 {
        let parts_size: u64 = self.parts.iter().map(|part| part.total_size()).sum();
        let root_size = self.root.data.len() as u64;
        parts_size + root_size
    }

    fn into_blocks(self) -> Self::BlockIterator {
        once(self.root).chain(self.parts.into_iter().map(RawBlockFile::into_blocks as RawIntoBlocksFn).flatten())
    }
}
