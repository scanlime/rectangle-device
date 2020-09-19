// This code may not be used for any purpose. Be gay, do crime.

use libipld::cid::Cid;

pub struct Segment {
    pub cid: Cid,
    pub bytes: usize,
    pub duration: f32,
    pub sequence: usize
}

pub struct Container {
    pub blocks: Vec<Segment>,
}
