// This code may not be used for any purpose. Be gay, do crime.

use libipld::Ipld;
use crate::blocks::dag;

// Protocol buffer tags
const PBTAG_TYPE: u8 = 8;
const PBTAG_TOTAL_SIZE: u8 = 24;
const PBTAG_BLOCK_SIZE: u8 = 32;

// Hardcoded enum values from the UnixFS protocol spec
const TYPE_DIRECTORY: u8 = 1;
const TYPE_FILE: u8 = 2;

pub fn make_directory(links: Vec<Ipld>) -> Ipld {
    // Minimum viable UnixFS directory node.
    dag::make_pb_node(links, vec![PBTAG_TYPE, TYPE_DIRECTORY])
}

pub fn make_file(links: Vec<Ipld>, sizes: Vec<usize>) -> Ipld {
    // Minimum viable UnixFS file node.
    let mut data = vec![PBTAG_TYPE, TYPE_FILE];
    let total_size: usize = sizes.iter().sum();
    data.push(PBTAG_TOTAL_SIZE);
    leb128::write::unsigned(&mut data, total_size as u64).unwrap();
    for size in sizes {
        data.push(PBTAG_BLOCK_SIZE);
        leb128::write::unsigned(&mut data, size as u64).unwrap();
    }
    dag::make_pb_node(links, data)
}
