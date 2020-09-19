// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::Receiver;
use libipld::cid::Cid;
use reqwest::{Client, StatusCode};
use crate::config;

pub struct Warmer {
    pub cid_receiver: Receiver<Cid>,
}

const POOL_SIZE: usize = 80;
const NUM_RETRIES: usize = 10;

impl Warmer {
    pub async fn task(self) {
        let mut tasks = vec![];
        for warmer_id in 0..POOL_SIZE {
            let cid_receiver = self.cid_receiver.clone();
            let client = Client::new();
            tasks.push(tokio::spawn(async move {
                loop {
                    let cid = cid_receiver.recv().await.unwrap();
                    let cid_str = cid.to_string();
                    let url = format!("https://{}/ipfs/{}", config::IPFS_GATEWAY, cid_str);
                    for try_num in 0..NUM_RETRIES {
                        log::trace!("[{}] head {} try {}", warmer_id, url, try_num);
                        let result = client.head(&url).send().await;
                        match result.map(|r| r.status()) {
                            Ok(StatusCode::OK) => {
                                log::info!("[{}] try# {}, {}", warmer_id, try_num, cid_str);
                                break;
                            },
                            err => {
                                log::trace!("[{}] {:?}", warmer_id, err);
                            }
                        };
                    }
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
    }
}
