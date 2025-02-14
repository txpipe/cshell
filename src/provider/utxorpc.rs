use miette::IntoDiagnostic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utxorpc::{CardanoSyncClient, ClientBuilder};

use crate::utils::Name;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct UTxORPCProvider {
    pub name: Name,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    pub is_default: Option<bool>,
    pub is_testnet: Option<bool>,
}

impl UTxORPCProvider {
    pub async fn test(&self) -> miette::Result<()> {
        println!("Building client...");
        println!("url: {}", self.url);
        let mut client_builder = ClientBuilder::new()
            .uri(self.url.clone())
            .into_diagnostic()?;

        if let Some(headers) = &self.headers {
            for (k, v) in headers {
                client_builder = client_builder.metadata(k, v).into_diagnostic()?;
            }
        }
        let mut client = client_builder.build::<CardanoSyncClient>().await;

        println!("Executing ReadTip method...");
        let result = client.read_tip().await.into_diagnostic()?;
        match result {
            Some(blockref) => {
                println!(
                    "Successfull request, block tip at slot {} and hash {}.",
                    blockref.index,
                    hex::encode(blockref.hash)
                )
            }
            None => println!("Successfull request"),
        }

        Ok(())
    }
}
