use url::Url;
use libp2p::Multiaddr;

#[derive(Clone, Debug)]
pub struct P2PConfig {
    pub pinning_services: Vec<Url>,
    pub pinning_gateways: Vec<Url>,
    pub public_gateways: Vec<Url>,
    pub router_peers: Vec<Multiaddr>,
    pub additional_peers: Vec<Multiaddr>,
    pub listen_addrs: Vec<Multiaddr>,
}
