// This code may not be used for any purpose. Be gay, do crime.

use libipld::cid::Cid;
use libipld::Ipld;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Link {
    pub cid: Cid,
    pub name: String,
    pub size: usize,
}

pub fn make_pb_link(link: Link) -> Ipld {
    let mut pb_link = BTreeMap::<String, Ipld>::new();
    pb_link.insert("Hash".to_string(), link.cid.into());
    pb_link.insert("Name".to_string(), link.name.into());
    pb_link.insert("Tsize".to_string(), link.size.into());
    pb_link.into()
}

pub fn make_pb_node(links: Vec<Ipld>, data: Vec<u8>) -> Ipld {
    let mut pb_node = BTreeMap::<String, Ipld>::new();
    pb_node.insert("Links".to_string(), links.into());
    pb_node.insert("Data".to_string(), data.into());
    pb_node.into()
}
