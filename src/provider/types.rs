use comfy_table::Table;
use pallas::ledger::addresses::Address;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::output::OutputFormatter;
use crate::provider::utxorpc::UTxORPCProvider;
use crate::types::{Balance, DetailedBalance};

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(tag = "type")]
pub enum Provider {
    UTxORPC(UTxORPCProvider),
}

impl Provider {
    pub fn name(&self) -> String {
        match self {
            Provider::UTxORPC(provider) => provider.name.to_string(),
        }
    }
    pub fn kind(&self) -> String {
        match self {
            Provider::UTxORPC(_) => "utxorpc".to_string(),
        }
    }
    pub fn parameters(&self) -> Option<Value> {
        match self {
            Provider::UTxORPC(provider) => Some(json!({
                "url": provider.url,
                "headers": provider.headers
            })),
        }
    }
    pub fn is_default(&self) -> bool {
        match self {
            Provider::UTxORPC(provider) => provider.is_default.unwrap_or(false),
        }
    }

    pub fn is_testnet(&self) -> bool {
        match self {
            Provider::UTxORPC(provider) => provider.is_testnet.unwrap_or(false),
        }
    }

    pub async fn test(&self) -> miette::Result<()> {
        match self {
            Provider::UTxORPC(provider) => provider.test().await,
        }
    }

    pub async fn get_balance(&self, address: &Address) -> miette::Result<Balance> {
        match self {
            Provider::UTxORPC(provider) => provider.get_balance(address).await,
        }
    }

    pub async fn get_detailed_balance(&self, address: &Address) -> miette::Result<DetailedBalance> {
        match self {
            Provider::UTxORPC(provider) => provider.get_detailed_balance(address).await,
        }
    }

    pub async fn submit(&self, tx: &[u8]) -> miette::Result<Vec<u8>> {
        match self {
            Provider::UTxORPC(provider) => provider.submit(tx).await,
        }
    }
}

impl OutputFormatter for Provider {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec![
            "Name",
            "Kind",
            "is testnet?",
            "Is default?",
            "Parameters",
        ]);
        table.add_row(vec![
            self.name(),
            self.kind(),
            self.is_testnet().to_string(),
            self.is_default().to_string(),
            match self.parameters() {
                Some(value) => serde_json::to_string(&value).unwrap(),
                None => "".to_string(),
            },
        ]);

        println!("{}", table);
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}

impl OutputFormatter for &Vec<Provider> {
    fn to_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Name", "Kind", "Is default?", "Parameters"]);
        for provider in *self {
            table.add_row(vec![
                provider.name(),
                provider.kind(),
                provider.is_default().to_string(),
                match provider.parameters() {
                    Some(value) => serde_json::to_string(&value).unwrap(),
                    None => "".to_string(),
                },
            ]);
        }
        println!("{}", table);
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}
