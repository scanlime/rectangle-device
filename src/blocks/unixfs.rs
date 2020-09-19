// This code may not be used for any purpose. Be gay, do crime.

use libipld::Ipld;
use crate::blocks::dag;

// Hardcoded protocol buffer field tag for 'type'
const PBTAG_TYPE: u8 = 8;

// Hardcoded enum values from the UnixFS protocol spec
const TYPE_DIRECTORY: u8 = 1;
const TYPE_FILE: u8 = 2;

pub fn make_directory(links: Vec<Ipld>) -> Ipld {
    // Minimum viable UnixFS directory node.
    // There are many optional fields we leave out, for timestamps and modes and such.
    dag::make_pb_node(links, vec![PBTAG_TYPE, TYPE_DIRECTORY])
}

pub fn make_file(links: Vec<Ipld>) -> Ipld {
    // Minimum viable UnixFS file node.
    // Leaving out lots of optional fields, including the duplicate block list which
    // go-ipfs still writes here but doesn't actually need. (It uses the one in the
    // DAG node for seeking now, not this one.)
    dag::make_pb_node(links, vec![PBTAG_TYPE, TYPE_DIRECTORY])
}
