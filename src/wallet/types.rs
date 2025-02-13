use crate::utils::Name;
use chrono::{DateTime, Local};
use comfy_table::Table;
use pallas::{
    crypto::{hash::Hash, key::ed25519::SecretKey},
    ledger::{
        addresses::{Address, Network, ShelleyAddress, ShelleyDelegationPart, ShelleyPaymentPart},
        traverse::ComputeHash,
    },
};
use pallas_wallet::wrapper::encrypt_private_key;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

use crate::{output::OutputFormatter, utils};

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
    pub fn try_from(name: &str, password: &str, is_default: bool) -> miette::Result<Self> {
        let private_key = SecretKey::new(OsRng);
        let public_key = private_key.public_key().compute_hash().to_vec();

        let encrypted_private_key =
            encrypt_private_key(OsRng, private_key.into(), &password.to_string());

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
        let hash: Hash<28> = Hash::from(self.public_key.as_slice());
        if is_testnet {
            ShelleyAddress::new(
                Network::Testnet,
                ShelleyPaymentPart::key_hash(hash),
                ShelleyDelegationPart::Null,
            )
            .into()
        } else {
            ShelleyAddress::new(
                Network::Mainnet,
                ShelleyPaymentPart::key_hash(hash),
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
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}

impl OutputFormatter for &Vec<Wallet> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Name", "Created", "Modified"]);

        for wallet in self.iter() {
            table.add_row(vec![
                wallet.name.to_string(),
                utils::pretty_print_date(&wallet.created),
                utils::pretty_print_date(&wallet.modified),
            ]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json: String = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}
