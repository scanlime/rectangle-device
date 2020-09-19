// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::Receiver;
use std::time::Duration;
use reqwest::{Client, StatusCode};

#[derive(Clone)]
pub struct Warmer {
    pub url_receiver: Receiver<String>,
}

const POOL_SIZE: usize = 500;
const NUM_RETRIES: usize = 100;
const TIMEOUT_MSEC: u64 = 3000;

impl Warmer {
    pub async fn task(self) {
        let mut tasks = vec![];
        for pool_id in 0..POOL_SIZE {
            let pool_clone = self.clone();
            tasks.push(tokio::spawn(async move {
                pool_clone.pool_task(pool_id).await;
            }))
        }
        for task in tasks {
            task.await.unwrap();
        }
    }

    async fn pool_task(&self, pool_id: usize) {
        let client = Client::builder()
            .timeout(Duration::from_millis(TIMEOUT_MSEC))
            .build().unwrap();

        loop {
            self.pool_task_poll(pool_id, &client).await;
        }
    }

    async fn pool_task_poll(&self, pool_id: usize, client: &Client) {
        let url = self.url_receiver.recv().await.unwrap();
        for try_num in 0..NUM_RETRIES {
            log::trace!("[{}] head {} try {}", pool_id, url, try_num);
            let result = client.head(&url).send().await;
            match result.map(|r| r.status()) {
                Ok(StatusCode::OK) => {
                    log::info!("[{}] try# {}, {}", pool_id, try_num, url);
                    return;
                },
                err => {
                    log::trace!("[{}] {:?}", pool_id, err);
                }
            };
        }
        log::warn!("[{}] failed {} after {} tries", pool_id, url, NUM_RETRIES);
    }
}
