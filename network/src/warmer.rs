// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::{channel, Sender, Receiver, TrySendError};
use std::time::Duration;
use reqwest::{Client, StatusCode, Url};

#[derive(Debug)]
struct QueueItem {
    url: Url,
    try_num: u64,
}

#[derive(Clone)]
pub struct Warmer {
    sender: Sender<QueueItem>,
    receiver: Receiver<QueueItem>,
}

const POOL_SIZE: usize = 10;
const QUEUE_SIZE: usize = 10000;
const TIMEOUT_MSEC: u64 = 500;
const NUM_RETRIES: u64 = 10;

impl Warmer {
    pub fn new() -> Warmer {
        let (sender, receiver) = channel(QUEUE_SIZE);
        Warmer { sender, receiver }
    }

    pub fn send(&self, url: Url) {
        match self.sender.try_send(QueueItem {
            url,
            try_num: 0
        }) {
            Ok(()) => {},
            Err(TrySendError::Full(item)) => {
                log::error!("queue full, dropping {:?}", item);
            },
            Err(TrySendError::Disconnected(item)) => {
                log::error!("queue disconnected, dropping {:?}", item);
            },
        }
    }

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
            let item = self.receiver.recv().await.unwrap();
            log::trace!("[{}] head {} try {}", pool_id, item.url, item.try_num);

            let result = client.head(item.url.clone()).send().await;
            match result.map(|r| r.status()) {
                Ok(StatusCode::OK) => {
                    log::debug!("[{}] try# {}, {}", pool_id, item.try_num, item.url);
                },
                err => {
                    let next_try = QueueItem {
                        url: item.url,
                        try_num: item.try_num + 1,
                    };
                    if next_try.try_num > NUM_RETRIES {
                        log::error!("[{}] failed {} after {} tries", pool_id, next_try.url, item.try_num);
                    } else {
                        log::trace!("[{}] {:?}, retrying later", pool_id, err);
                        self.sender.send(next_try).await;
                    }
                }
            }
        }
    }
}
