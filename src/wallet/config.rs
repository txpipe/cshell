use crate::{
    dirs,
    utils::{self, Config, ConfigName},
    utxorpc::config::Utxorpc,
};
use chrono::{DateTime, Local};
use comfy_table::Table;
use futures::Future;
use miette::{Context, IntoDiagnostic};
use pallas::ledger::{
    addresses::ShelleyAddress,
    traverse::{Era, MultiEraOutput},
};
use serde::{Deserialize, Serialize};

use crate::utils::OutputFormatter;
use entity::utxo::Model as UtxoModel;

#[derive(Debug, Serialize, Deserialize)]
pub struct Addresses {
    pub mainnet: String,
    pub testnet: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Keys {
    pub public_key_hash: String,
    pub private_encrypted: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub version: String,
    pub name: ConfigName,
    pub keys: Keys,
    pub addresses: Addresses,
    pub utxorpc_config: ConfigName,
    pub created_on: DateTime<Local>,
    pub last_updated: DateTime<Local>,
}

impl Wallet {
    pub fn new(
        name: ConfigName,
        keys: Keys,
        addresses: Addresses,
        utxorpc_config: ConfigName,
    ) -> miette::Result<Self> {
        let now = Local::now();
        Ok(Self {
            version: crate::utils::VERSION.to_owned(),
            name,
            keys,
            addresses,
            utxorpc_config,
            created_on: now,
            last_updated: now,
        })
    }

    pub fn update(
        &mut self,
        keys: Option<Keys>,
        addresses: Option<Addresses>,
        utxorpc_config: Option<ConfigName>,
    ) {
        if let Some(keys) = keys {
            self.keys = keys;
        }
        if let Some(addresses) = addresses {
            self.addresses = addresses;
        }
        if let Some(utxorpc_config) = utxorpc_config {
            self.utxorpc_config = utxorpc_config;
        }
    }

    pub fn address(&self, utxo_config: &Utxorpc) -> &str {
        if utxo_config.is_testnet {
            &self.addresses.testnet
        } else {
            &self.addresses.mainnet
        }
    }
}
impl Config for Wallet {
    fn name(&self) -> &ConfigName {
        &self.name
    }

    fn config_type() -> &'static str {
        "Wallet"
    }

    fn parent_dir_name() -> &'static str {
        &dirs::WALLETS_PARENT_DIR
    }

    fn toml_file_name() -> &'static str {
        &dirs::WALLET_CONFIG_FILENAME
    }
}

impl OutputFormatter for Wallet {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Property", "Value"]);

        table.add_row(vec!["Name", &self.name]);
        table.add_row(vec!["UTxO RPC Config", &self.utxorpc_config]);
        table.add_row(vec!["Public Key Hash", &self.keys.public_key_hash]);
        table.add_row(vec!["Address (mainnet)", &self.addresses.mainnet]);
        table.add_row(vec!["Address (testnet)", &self.addresses.testnet]);
        table.add_row(vec![
            "Created on",
            &utils::pretty_print_date(&self.created_on),
        ]);
        table.add_row(vec![
            "Last updated",
            &utils::pretty_print_date(&self.last_updated),
        ]);

        println!("{table}");
    }

    fn to_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}

impl OutputFormatter for Vec<Wallet> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Name", "UTxO RPC Config"]);

        for wallet in self {
            table.add_row(vec![&wallet.name.raw, &wallet.utxorpc_config.raw]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json: String = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}

#[derive(Debug, Serialize)]
pub struct BalanceView {
    pub lovelace: u64,
    pub tokens: Vec<(String, u64)>,
}

impl BalanceView {
    pub fn new(lovelace: u64, tokens: Vec<(String, u64)>) -> Self {
        Self { lovelace, tokens }
    }
}

impl OutputFormatter for BalanceView {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["token", "amount"]);

        table.add_row(vec!["lovelace".to_string(), self.lovelace.to_string()]);

        for (token, amount) in &self.tokens {
            table.add_row(vec![token, &amount.to_string()]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}
