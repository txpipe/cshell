use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local};
use comfy_table::Table;
use miette::{Context, IntoDiagnostic};
use pallas::ledger::traverse::{Era, MultiEraOutput};
use serde::{Deserialize, Serialize};

use super::dal::entities::utxo::Model as UtxoModel;
use crate::utils::{deserialize_date, serialize_date, OutputFormatter};

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
pub struct SubmitApi {
    pub url: String,
    pub headers: HashMap<String, String>,
}

const DEFAULT_SUBMIT_API_URL: &str =
    "https://submitapi-preprod-api-peaceful-relation-132bd4.us1.demeter.run/api/submit/tx";

impl Default for SubmitApi {
    fn default() -> Self {
        Self {
            url: String::from(DEFAULT_SUBMIT_API_URL),
            headers: HashMap::default(),
        }
    }
}

const WALLETS_DIR_NAME: &str = "wallets";
const CONFIG_FILENAME: &str = "config.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub version: String,
    pub name: String,
    // TODO
    pub keys: Keys,
    pub addresses: Addresses,
    pub chain: Option<String>,
    pub submit_api: Option<SubmitApi>,

    #[serde(serialize_with = "serialize_date")]
    #[serde(deserialize_with = "deserialize_date")]
    pub created_on: DateTime<Local>,
}

impl Wallet {
    pub fn new(name: String, keys: Keys, addresses: Addresses, chain: Option<String>) -> Self {
        let version = String::from("v1alpha");

        Self {
            version,
            name,
            // TODO
            keys,
            addresses,
            chain,
            submit_api: None,
            created_on: Local::now(),
        }
    }

    pub fn wallet_dir(root_dir: &Path, name: &str) -> PathBuf {
        root_dir.join(WALLETS_DIR_NAME).join(name)
    }

    pub fn load_config(root_dir: &Path, name: &str) -> miette::Result<Option<Self>> {
        let config_path = Self::config_path(root_dir, name);

        if config_path.exists() {
            let file = fs::File::open(config_path).into_diagnostic()?;
            let contents = {
                let mut contents = String::new();
                let mut buf_reader = BufReader::new(file);
                buf_reader.read_to_string(&mut contents).into_diagnostic()?;
                contents
            };

            let wallet: Wallet = toml::from_str(&contents).into_diagnostic()?;
            return Ok(Some(wallet));
        }

        Ok(None)
    }

    pub fn config_path(root_dir: &Path, name: &str) -> PathBuf {
        Self::wallet_dir(root_dir, name).join(CONFIG_FILENAME)
    }

    pub fn list_available(root_dir: &Path) -> miette::Result<Vec<String>> {
        let parent = root_dir
            .join(WALLETS_DIR_NAME)
            .read_dir()
            .into_diagnostic()?;

        let names = parent
            .into_iter()
            .filter_map(|dir| dir.ok())
            .map(|d| String::from(d.file_name().to_string_lossy()))
            .collect();

        Ok(names)
    }

    pub fn save_config(&self, root_dir: &Path) -> miette::Result<()> {
        let wallet_path = Self::wallet_dir(root_dir, &self.name);
        fs::create_dir_all(wallet_path).into_diagnostic()?;

        let config_path = Self::config_path(root_dir, &self.name);
        let toml_string = toml::to_string(self).into_diagnostic()?;
        let mut file = fs::File::create(config_path).into_diagnostic()?;
        file.write_all(toml_string.as_bytes()).into_diagnostic()?;
        Ok(())
    }
}

impl OutputFormatter for Wallet {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["property", "value"]);

        table.add_row(vec!["Name", &self.name]);
        table.add_row(vec![
            "Chain",
            &self.chain.as_deref().unwrap_or("not attached"),
        ]);
        table.add_row(vec!["Public Key Hash", &self.keys.public_key_hash]);
        table.add_row(vec!["Address (mainnet)", &self.addresses.mainnet]);
        table.add_row(vec!["Address (testnet)", &self.addresses.testnet]);

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

        table.set_header(vec!["name", "chain"]);

        for wallet in self {
            table.add_row(vec![
                &wallet.name,
                wallet
                    .chain
                    .as_ref()
                    .unwrap_or(&String::from("not attached")),
            ]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
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
