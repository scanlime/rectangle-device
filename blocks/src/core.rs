// This code may not be used for any purpose. Be gay, do crime.

use libipld::codec_impl::Multicodec;
use libipld::multihash::Multihash;

// This is an architectural limit in IPFS, chosen to limit the amount of memory
// needed by bitswap to buffer not-yet-verified data from peers.
pub const BLOCK_MAX_BYTES : usize = 1024 * 1024;

pub use libipld::multihash::SHA2_256 as DefaultHashType;

pub type Block = libipld::block::Block<Multicodec, Multihash>;
pub use libipld::cid::Cid;
pub use libipld::pb::PbLink;

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

pub struct BlockInfo {
    pub block: Block,
    pub usage: BlockUsage,
}

impl BlockUsage {
    pub fn attach_to(&self, block: Block) -> BlockInfo {
        BlockInfo {
            block: block,
            usage: self.clone(),
        }
    }
}
