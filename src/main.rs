use libipld::block::Block;
use libipld::raw::RawCodec;
use libipld::codec_impl::Multicodec;
use libipld::multihash::{Multihash, SHA2_256};
use multibase::Base;
use std::time::Duration;
use async_std::task;
use ipfs_embed::{TREE, Config, Store, WritableStore};

fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let db = sled::open("db")?;
    let tree_name = TREE.to_string();
    let tree = db.open_tree(tree_name)?;
    let config = Config::new(tree, Default::default());
    let store = Store::<Multicodec, Multihash>::new(config)?;

    println!("this is {}/p2p/{}", store.address(), store.peer_id());

    task::block_on(async {

        let mut block_data: Vec<u8> = vec![];

        loop {
            let block = Block::encode(RawCodec, SHA2_256, &block_data)?;
            let cid_str = block.cid.to_string_of_base(Base::Base32Lower)?;
            println!("storing {:?}", cid_str);
            store.insert(&block).await?;
            block_data.push(1);
            task::sleep(Duration::from_secs(1)).await;
        }
    })
}
