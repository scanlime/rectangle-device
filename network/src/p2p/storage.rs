use libp2p::PeerId;
use rectangle_device_blocks::{BlockInfo, BlockUsage, Cid};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

pub struct BlockStore {
    pub data: BTreeMap<Vec<u8>, BlockInfo>,
    send_queue: BTreeSet<BlockSendRequest>,
}

impl BlockStore {
    pub fn new() -> BlockStore {
        BlockStore {
            data: BTreeMap::new(),
            send_queue: BTreeSet::new(),
        }
    }

    pub fn enqueue_send(&mut self, cid: Cid, peer_id: PeerId, usage: BlockUsage) {
        self.send_queue.insert(BlockSendRequest {
            usage,
            cid,
            peer_id,
        });
    }

    pub fn next_send(&mut self) -> Option<BlockSendRequest> {
        match self.send_queue.iter().cloned().next() {
            None => None,
            Some(req) => {
                self.send_queue.remove(&req);
                Some(req)
            }
        }
    }
}

#[derive(Eq, Debug, Clone)]
pub struct BlockSendRequest {
    pub usage: BlockUsage,
    pub cid: Cid,
    pub peer_id: PeerId,
}

impl Ord for BlockSendRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.usage.cmp(&other.usage)
    }
}

impl PartialOrd for BlockSendRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for BlockSendRequest {
    fn eq(&self, other: &Self) -> bool {
        self.usage == other.usage
    }
}
