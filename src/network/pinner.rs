// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::Receiver;
use std::error::Error;
use serde::{Deserialize, Serialize};
use libipld::cid::Cid;

const POOL_SIZE: usize = 2;
const QUEUE_SIZE: usize = 100;

#[derive(Debug)]
struct QueueItem {
    // https://ipfs.github.io/pinning-services-api-spec/
    api: String,
    pin: APIPin,
}

#[derive(Clone)]
pub struct Pinner {
    sender: Sender<QueueItem>,
    receiver: Receiver<QueueItem>,
}

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
    pub fn new(pinning_api: String, local_multiaddrs: Vec<String>) -> self {
        let (sender, receiver) = channel(128);
        Pinner { sender, receiver }
    }

    pub fn send(&self, api: String, cid: String, name: String, origins: Vec<String>) {
        match self.sender.try_send(QueueItem {
            url,
            try_num: 0
        }) {
            Ok(()) => {},
            Err(TrySendError::Full(item)) => log::error!("queue full, dropping {:?}", item),
            Err(TrySendError::Disconnected(item)) => log::error!("queue disconnected, dropping {:?}", item),
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


    pub async fn task(self) {
        let client = reqwest::Client::new();
        let mut id = None;

        loop {
            let cid = self.pin_receiver.recv().await.unwrap();
            let pin = APIPin {
                cid: cid.to_string(),
                name: "temporary name for pinning request".to_string(),
                origins: self.local_multiaddrs.clone()
            };

            let url = match &id {
                None => format!("{}/pins", self.pinning_api),
                Some(id) => format!("{}/pins/{}", self.pinning_api, id)
            };

            let result = async {
                let result = client.post(&url).json(&pin).send().await?;
                let status: APIPinStatus = result.json().await?;
                log::info!("pinning api at {} says {:?}", url, status);
                Result::<String, Box<dyn Error>>::Ok(status.id)
            }.await;

            id = match result {
                Err(err) => {
                    log::warn!("pinning api error, {}", err);
                    None
                },
                Ok(new_id) => Some(new_id),
            };
        }
    }
}
