use comfy_table::Table;
use serde::{Deserialize, Serialize};

use crate::output::OutputFormatter;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Asset {
    #[serde(with = "hex::serde")]
    pub name: Vec<u8>,
    pub quantity: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Datum {
    #[serde(with = "hex::serde")]
    pub hash: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BalanceAsset {
    #[serde(with = "hex::serde")]
    pub policy_id: Vec<u8>,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct UTxO {
    #[serde(with = "hex::serde")]
    pub tx: Vec<u8>,
    pub tx_index: u64,
    pub address: String,
    pub coin: String, // To avoid overflow
    pub assets: Vec<BalanceAsset>,
    pub datum: Option<Datum>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Balance {
    pub address: String,
    pub coin: String, // To avoid overflow
    pub assets: Vec<BalanceAsset>,
    pub datums: Vec<Datum>,
}

pub type DetailedBalance = Vec<UTxO>;

impl OutputFormatter for Balance {
    fn to_table(&self) {
        println!("Balance for address: {}", self.address);
        println!("  Lovelace: {} ADA", self.coin);
        if !self.assets.is_empty() {
            println!();
            println!("Assets:");

            let mut table = Table::new();
            table.set_header(vec!["Policy", "Asset", "Quantity"]);

            for entry in &self.assets {
                for asset in &entry.assets {
                    table.add_row(vec![
                        hex::encode(&entry.policy_id),
                        hex::encode(&asset.name),
                        asset.quantity.clone(),
                    ]);
                }
            }
            println!("{table}");
        }

        if !self.datums.is_empty() {
            println!();
            println!("{:?}", self.datums);
            println!("Datums:");

            let mut table = Table::new();
            table.set_header(vec!["Datum hash"]);

            for datum in &self.datums {
                table.add_row(vec![hex::encode(&datum.hash)]);
            }
            println!("{table}");
        }
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}

impl OutputFormatter for DetailedBalance {
    fn to_table(&self) {
        if !self.is_empty() {
            println!("UTxOs");
            println!("=====");
        }
        for utxo in self {
            println!();
            println!("* {}#{}", hex::encode(&utxo.tx), utxo.tx_index);
            println!("  * Lovelace: {}", utxo.coin);

            if let Some(datum) = &utxo.datum {
                println!("  * Datum: {}", hex::encode(datum.hash.clone()));
            }

            if !utxo.assets.is_empty() {
                println!();
                println!("  * Assets:");

                let mut table = Table::new();
                table.set_header(vec!["Policy", "Asset", "Quantity"]);

                for entry in &utxo.assets {
                    for asset in &entry.assets {
                        table.add_row(vec![
                            hex::encode(&entry.policy_id),
                            hex::encode(&asset.name),
                            asset.quantity.clone(),
                        ]);
                    }
                }
                println!("{table}");
            }
        }
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}
