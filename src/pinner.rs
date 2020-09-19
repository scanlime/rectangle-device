// This code may not be used for any purpose. Be gay, do crime.

use async_std::sync::Receiver;
use std::error::Error;
use serde::{Deserialize, Serialize};
use libipld::cid::Cid;
use crate::config;

pub struct Pinner {
    pub pin_receiver: Receiver<Cid>,
    pub local_multiaddrs: Vec<String>
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
    pub async fn task(self) {
        let client = reqwest::Client::new();
        let mut id = None;

        loop {
            let cid = self.pin_receiver.recv().await.unwrap();
            let pin = APIPin {
                cid: cid.to_string(),
                name: config::IPFS_PINNING_NAME.to_string(),
                origins: self.local_multiaddrs.clone()
            };

            let url = match &id {
                None => format!("{}/pins", config::IPFS_PINNING_API),
                Some(id) => format!("{}/pins/{}", config::IPFS_PINNING_API, id)
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
