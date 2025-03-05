use crate::utils::Name;
use chrono::{DateTime, Local};
use comfy_table::Table;
use miette::IntoDiagnostic;
use pallas::ledger::{
    addresses::{Address, Network, ShelleyAddress, ShelleyDelegationPart, ShelleyPaymentPart},
    traverse::ComputeHash,
};
use pallas_wallet::{
    hd::{Bip32PrivateKey, Bip32PublicKey},
    wrapper::encrypt_private_key,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{output::OutputFormatter, utils};

pub type NewWallet = (String, Wallet);

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Wallet {
    pub name: Name,
    #[serde(with = "hex::serde")]
    pub public_key: Vec<u8>,
    #[serde(with = "hex::serde")]
    pub encrypted_private_key: Vec<u8>,
    pub created: DateTime<Local>,
    pub modified: DateTime<Local>,
    pub is_default: bool,
}

impl Wallet {
    pub fn try_from(name: &str, password: &str, is_default: bool) -> miette::Result<NewWallet> {
        let (private_key, mnemonic) =
            Bip32PrivateKey::generate_with_mnemonic(OsRng, password.to_string());
        let public_key = private_key.to_public().as_bytes();

        let encrypted_private_key = encrypt_private_key(
            OsRng,
            private_key.to_ed25519_private_key(),
            &password.to_string(),
        );

        Ok((
            mnemonic.to_string(),
            Self {
                name: Name::try_from(name)?,
                encrypted_private_key,
                public_key,
                created: Local::now(),
                modified: Local::now(),
                is_default,
            },
        ))
    }

    pub fn try_from_mnemonic(
        name: &str,
        password: &str,
        mnemonic: &str,
        is_default: bool,
    ) -> miette::Result<Self> {
        let private_key =
            Bip32PrivateKey::from_bip39_mnenomic(mnemonic.to_string(), password.to_string())
                .into_diagnostic()?;
        let public_key = private_key.to_public().as_bytes();

        let encrypted_private_key = encrypt_private_key(
            OsRng,
            private_key.to_ed25519_private_key(),
            &password.to_string(),
        );

        Ok(Self {
            name: Name::try_from(name)?,
            encrypted_private_key,
            public_key,
            created: Local::now(),
            modified: Local::now(),
            is_default,
        })
    }

    pub fn address(&self, is_testnet: bool) -> Address {
        let pk = Bip32PublicKey::from_bytes(self.public_key.clone().try_into().unwrap())
            .to_ed25519_pubkey();
        if is_testnet {
            ShelleyAddress::new(
                Network::Testnet,
                ShelleyPaymentPart::key_hash(pk.compute_hash()),
                ShelleyDelegationPart::Null,
            )
            .into()
        } else {
            ShelleyAddress::new(
                Network::Mainnet,
                ShelleyPaymentPart::key_hash(pk.compute_hash()),
                ShelleyDelegationPart::Null,
            )
            .into()
        }
    }
}

impl OutputFormatter for Wallet {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Property", "Value"]);

        table.add_row(vec!["Name", &self.name]);
        table.add_row(vec!["Public Key Hash", &hex::encode(&self.public_key)]);
        table.add_row(vec!["Address (mainnet)", &self.address(false).to_string()]);
        table.add_row(vec!["Address (testnet)", &self.address(true).to_string()]);
        table.add_row(vec!["Created", &utils::pretty_print_date(&self.created)]);
        table.add_row(vec!["Modified", &utils::pretty_print_date(&self.modified)]);

        println!("{table}");
    }

    fn to_json(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "name": &self.name,
                "public_key": hex::encode(&self.public_key),
                "addresses": {
                    "mainnet": &self.address(false).to_string(),
                    "testnet": &self.address(true).to_string(),
                },
                "created": self.created,
                "modified": self.modified,
                "is_default": self.is_default,
            }))
            .unwrap()
        );
    }
}

impl OutputFormatter for &Vec<Wallet> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Name", "Created", "Modified", "Is Default?"]);

        for wallet in self.iter() {
            table.add_row(vec![
                wallet.name.to_string(),
                utils::pretty_print_date(&wallet.created),
                utils::pretty_print_date(&wallet.modified),
                wallet.is_default.to_string(),
            ]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &self
                    .iter()
                    .map(|wallet| {
                        json!({
                            "name": &wallet.name,
                            "public_key": hex::encode(&wallet.public_key),
                            "addresses": {
                                "mainnet": &wallet.address(false).to_string(),
                                "testnet": &wallet.address(true).to_string(),
                            },
                            "created": wallet.created,
                            "modified": wallet.modified,
                            "is_default": wallet.is_default,
                        })
                    })
                    .collect::<Vec<Value>>(),
            )
            .unwrap()
        );
    }
}

impl OutputFormatter for NewWallet {
    fn to_table(&self) {
        println!("Your mnemonic phrase is the following:");
        println!("\n");
        println!("* {}", self.0);
        println!("\n");
        println!("Save this phrase somewhere safe to restore your wallet if it ever gets lost.");

        self.1.to_table();
    }

    fn to_json(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "name": &self.1.name,
                "mnemonic": &self.0,
                "public_key": hex::encode(&self.1.public_key),
                "addresses": {
                    "mainnet": &self.1.address(false).to_string(),
                    "testnet": &self.1.address(true).to_string(),
                },
                "created": self.1.created,
                "modified": self.1.modified,
                "is_default": self.1.is_default,
            }))
            .unwrap()
        );
    }
}
