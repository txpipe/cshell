use crate::{
    dirs,
    utils::{self, Config, ConfigName},
};
use chrono::{DateTime, Local};
use comfy_table::Table;
use miette::{Context, IntoDiagnostic};
use pallas::ledger::traverse::{Era, MultiEraOutput};
use serde::{Deserialize, Serialize};

use super::dal::entities::utxo::Model as UtxoModel;
use crate::utils::OutputFormatter;

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
}
impl Config for Wallet {
    fn name(&self) -> &ConfigName {
        &self.name
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
pub struct UtxoView {
    pub tx_hash: String,
    pub txo_index: i32,
    pub lovelace: u64,
    pub datum: bool,
    pub tokens: Vec<(String, u64)>,
}

impl OutputFormatter for Vec<UtxoView> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["tx hash", "txo index", "lovelace", "datum", "tokens"]);

        for utxo in self {
            let tokens = utxo
                .tokens
                .iter()
                .map(|t| format!("{} {}", t.1, t.0))
                .collect::<Vec<String>>()
                .join("\n");

            table.add_row(vec![
                &utxo.tx_hash,
                &utxo.txo_index.to_string(),
                &utxo.lovelace.to_string(),
                &utxo.datum.to_string(),
                &tokens,
            ]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}

impl TryFrom<UtxoModel> for UtxoView {
    type Error = miette::ErrReport;

    fn try_from(value: UtxoModel) -> Result<Self, Self::Error> {
        let era = Era::try_from(value.era)
            .into_diagnostic()
            .context("parsing era")?;

        let output = MultiEraOutput::decode(era, &value.cbor).into_diagnostic()?;

        let tx_hash = hex::encode(value.tx_hash);
        let txo_index = value.txo_index;

        let lovelace = output.lovelace_amount();
        let datum: bool = output.datum().is_some();
        let tokens: Vec<(String, u64)> = output
            .non_ada_assets()
            .iter()
            .flat_map(|p| {
                p.assets()
                    .iter()
                    .map(|a| {
                        (
                            a.to_ascii_name().unwrap_or_default(),
                            a.output_coin().unwrap_or_default(),
                        )
                    })
                    .collect::<Vec<(String, u64)>>()
            })
            .collect();

        let utxo_view = UtxoView {
            tx_hash,
            txo_index,
            lovelace,
            datum,
            tokens,
        };

        Ok(utxo_view)
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
