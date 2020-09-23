pub use askama::Template;

pub fn main_js() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/dist/main.js"))
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub main_js_cid: &'a str,
    pub ipfs_gateway: &'a str,
    pub ipfs_delegates: &'a str,
    pub ipfs_bootstrap: &'a str,
    pub hls_cid: &'a str,
    pub hls_name: &'a str,
}
