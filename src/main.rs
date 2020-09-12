use libipld::block::Block;
use libipld::raw::RawCodec;
use libipld::codec_impl::Multicodec;
use libipld::multihash::{Multihash, SHA2_256};
use multibase::Base;
use async_std::{io, task};
use async_std::io::ReadExt;
use ipfs_embed::{TREE, MAX_BLOCK_SIZE, Config, Store, WritableStore, NetworkConfig};

fn main() -> Result<(), Box<dyn std::error::Error>>
{
    env_logger::init();
    
    let db = sled::open("db")?;
    let tree_name = TREE.to_string();
    let tree = db.open_tree(tree_name)?;
    let mut network = NetworkConfig::new();

    network.boot_nodes.push((
        "/ip4/99.149.215.66/tcp/4001".parse()?,
        "QmPjtoXdQobBpWa2yS4rfmHVDoCbom2r2SMDTUa1Nk7kJ5".parse()?));

    let config = Config::new(tree, network);
    let store = Store::<Multicodec, Multihash>::new(config)?;
    let mut stdin = io::stdin();

    println!("serving {}/p2p/{}", store.address(), store.peer_id());

    task::block_on(async {
        let mut block_data: Vec<u8> = vec![0; MAX_BLOCK_SIZE];

        loop {
            stdin.read_exact(&mut block_data).await?;
            let block = Block::encode(RawCodec, SHA2_256, &block_data)?;
            let cid_str = block.cid.to_string_of_base(Base::Base32Lower)?;
            store.insert(&block).await?;
            println!("stored {}", cid_str);
        }
    })
}
