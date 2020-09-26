// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::{channel, Sender, Receiver, TrySendError};
use std::error::Error;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use reqwest::Client;

const POOL_SIZE: usize = 4;
const QUEUE_SIZE: usize = 100;
const TIMEOUT_MSEC: u64 = 10000;

#[derive(Debug)]
struct QueueItem {
    api: String,
    id: Option<String>,
    pin: APIPin,
}

#[derive(Clone)]
pub struct Pinner {
    sender: Sender<QueueItem>,
    receiver: Receiver<QueueItem>,
}

// https://ipfs.github.io/pinning-services-api-spec/
#[derive(Deserialize, Serialize, Debug)]
struct APIPin {
    cid: String,
    name: String,
    origins: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct APIPinStatus {
    id: String,
    status: String,
    created: String,
    pin: APIPin,
    delegates: Vec<String>,
}

impl Pinner {
    pub fn new() -> Self {
        let (sender, receiver) = channel(QUEUE_SIZE);
        Pinner { sender, receiver }
    }

    pub fn send(&self, api: String, cid: String, name: String, origins: Vec<String>) {
        match self.sender.try_send(QueueItem {
            api,
            // To do: reuse old pins, use pin completion to GC blocks from ram
            id: None,
            pin: APIPin {
                cid,
                name,
                origins
            }
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
            log::trace!("[{}] {:?}", pool_id, item);

            let url = match &item.id {
                None => format!("{}/pins", item.api),
                Some(id) => format!("{}/pins/{}", item.api, id)
            };

            let result = async {
                let result = client.post(&url).json(&item.pin).send().await?;
                let status: APIPinStatus = result.json().await?;
                log::info!("pinning api at {} says {:?}", url, status);
                Result::<String, Box<dyn Error>>::Ok(status.id)
            }.await;

            // to do: track pinning progress
            let _id = match result {
                Err(err) => {
                    log::warn!("pinning api error, {}", err);
                    None
                },
                Ok(new_id) => Some(new_id),
            };
        }
    }
}