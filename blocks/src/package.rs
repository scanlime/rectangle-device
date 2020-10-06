use crate::core::{Block, Cid, PbLink};

pub trait Package {
    type BlockIterator: Iterator<Item=Block>;

    fn cid(&self) -> &Cid;
    fn total_size(&self) -> u64;
    fn into_blocks(self) -> Self::BlockIterator;

    fn link(&self, name: String) -> PbLink {
        PbLink {
            cid: self.cid().clone(),
            size: self.total_size(),
            name
        }
    }
}
