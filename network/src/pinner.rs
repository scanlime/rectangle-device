use async_std::sync::{channel, Receiver, Sender, TrySendError};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::{error::Error, time::Duration};

const POOL_SIZE: usize = 4;
const QUEUE_SIZE: usize = 100;
const TIMEOUT_MSEC: u64 = 10000;

#[derive(Debug)]
struct QueueItem {
    api: Url,
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

    pub fn send(&self, api: Url, cid: String, name: String, origins: Vec<String>) {
        match self.sender.try_send(QueueItem {
            api,
            // To do: reuse old pins, use pin completion to GC blocks from ram
            id: None,
            pin: APIPin { cid, name, origins },
        }) {
            Ok(()) => {}
            Err(TrySendError::Full(item)) => {
                log::error!("queue full, dropping {:?}", item);
            }
            Err(TrySendError::Disconnected(item)) => {
                log::error!("queue disconnected, dropping {:?}", item);
            }
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
            .build()
            .unwrap();

        loop {
            let item = self.receiver.recv().await.unwrap();
            log::trace!("[{}] {:?}", pool_id, item);

            let pins = item.api.join("pins").unwrap();
            let url = match &item.id {
                None => pins,
                Some(id) => pins.join(&id.to_string()).unwrap(),
            };

            let result = async {
                log::info!("requesting {} {:?}", url, item.pin);
                let result = client.post(url).json(&item.pin).send().await?;
                let status: APIPinStatus = result.json().await?;
                log::info!("pinning api says {:?}", status);
                Result::<String, Box<dyn Error>>::Ok(status.id)
            }
            .await;

            // to do: track pinning progress
            let _id = match result {
                Err(err) => {
                    log::warn!("pinning api error, {}", err);
                    None
                }
                Ok(new_id) => Some(new_id),
            };
        }
    }
}
