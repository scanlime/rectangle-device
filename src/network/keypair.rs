// This code may not be used for any purpose. Be gay, do crime.

use crate::warmer::Warmer;
use rectangle_device_blocks::{BlockUsage, BlockInfo};
use async_std::sync::Receiver;
use core::pin::Pin;
use std::cmp::Ordering;
use futures::{Future, Stream};
use libipld::cid::Cid;
use async_std::task::{Poll, Context};
use libipld::multihash::Multihash;
use libp2p_bitswap::{Bitswap, BitswapEvent};
use libp2p::{PeerId, Swarm, NetworkBehaviour};
use libp2p::core::multiaddr::{Multiaddr, Protocol};
use libp2p::gossipsub::{self, Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, GossipsubEvent};
use libp2p::gossipsub::error::PublishError;
use libp2p::identity::Keypair;
use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::kad::{self, Kademlia, KademliaEvent, KademliaConfig};
use libp2p::kad::record::store::{MemoryStore, MemoryStoreConfig};
use libp2p::mdns::{Mdns, MdnsEvent};
use libp2p::ping::{Ping, PingConfig, PingEvent};
use libp2p::swarm::{SwarmEvent, NetworkBehaviourEventProcess, NetworkBehaviour};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::error::Error;
use std::process::{Command, Stdio};

fn keypair_from_openssl_rsa() -> Result<Keypair, Box<dyn Error>> {
    // This is a temporary hack.
    // I'm using an RSA keypair because the new elliptic curve
    // based identity seems to break p2p-circuit routing in go-ipfs?

    log::info!("generating key pair");

    let mut genpkey = Command::new("openssl")
        .arg("genpkey").arg("-algorithm").arg("RSA")
        .arg("-pkeyopt").arg("rsa_keygen_bits:2048")
        .arg("-pkeyopt").arg("rsa_keygen_pubexp:65537")
        .stdout(Stdio::piped())
        .spawn().unwrap();

    let pkcs8 = Command::new("openssl")
        .arg("pkcs8").arg("-topk8").arg("-nocrypt")
        .arg("-outform").arg("der")
        .stdin(Stdio::from(genpkey.stdout.take().unwrap()))
        .output().unwrap();

    let mut data = pkcs8.stdout.as_slice().to_vec();
    Ok(Keypair::rsa_from_pkcs8(&mut data)?)
}
