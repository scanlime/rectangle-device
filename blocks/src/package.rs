// This code may not be used for any purpose. Be gay, do crime.

use async_trait::async_trait;
use crate::core::{BlockInfo, BlockUsage, Cid, PbLink};

pub use async_std::sync::Sender;

#[async_trait]
pub trait Package {
    async fn send(self, sender: &Sender<BlockInfo>, usage: &BlockUsage);

    fn cid(&self) -> &Cid;
    fn total_size(&self) -> u64;

    fn link(&self, name: String) -> PbLink {
        PbLink {
            cid: self.cid().clone(),
            size: self.total_size(),
            name
        }
    }
}
