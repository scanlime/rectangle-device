// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::Receiver;
use std::time::Duration;
use reqwest::{Client, StatusCode};

pub struct Warmer {
    pub url_receiver: Receiver<String>,
}

const POOL_SIZE: usize = 100;
const NUM_RETRIES: usize = 20;
const TIMEOUT_MSEC: u64 = 500;

impl Warmer {
    pub async fn task(self) {
        let mut tasks = vec![];
        for warmer_id in 0..POOL_SIZE {
            let url_receiver = self.url_receiver.clone();
            let client = Client::builder()
                .timeout(Duration::from_millis(TIMEOUT_MSEC))
                .build().unwrap();
            tasks.push(tokio::spawn(async move {
                loop {
                    let url = url_receiver.recv().await.unwrap();
                    for try_num in 0..NUM_RETRIES {
                        log::trace!("[{}] head {} try {}", warmer_id, url, try_num);
                        let result = client.head(&url).send().await;
                        match result.map(|r| r.status()) {
                            Ok(StatusCode::OK) => {
                                log::debug!("[{}] try# {}, {}", warmer_id, try_num, url);
                                return;
                            },
                            err => {
                                log::trace!("[{}] {:?}", warmer_id, err);
                            }
                        };
                    }
                    log::warn!("[{}] failed {} after {} tries", warmer_id, url, NUM_RETRIES);
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
    }
}
