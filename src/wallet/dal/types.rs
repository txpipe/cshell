use std::error::Error;

use comfy_table::Table;
use entity::{block_history, recent_points, tx_history, utxo};
use miette::{bail, IntoDiagnostic};
use num_bigint::BigInt;
use pallas::{
    applying::utils::get_shelley_address,
    ledger::addresses::{Address, ShelleyAddress},
};
use prost::bytes::Bytes;
use serde::{Serialize, Serializer};
use utxorpc::spec::{
    cardano::{Block, TxInput, TxOutput},
    sync::BlockRef,
};

use crate::utils::OutputFormatter;

#[derive(Debug, Clone, Serialize)]
pub struct TransactionInfo {
    #[serde(serialize_with = "serialize_bytes_as_hex")]
    pub hash: Bytes,
    #[serde(serialize_with = "serialize_bytes_as_hex")]
    pub block_hash: Bytes,
    pub slot: u64,
    pub tx_index: u16,
    #[serde(serialize_with = "serialize_big_int")]
    pub delta: BigInt,
}
impl TransactionInfo {
    pub fn as_active_model(&self) -> tx_history::ActiveModel {
        entity::tx_history::ActiveModel {
            tx_hash: sea_orm::ActiveValue::Set(self.hash.to_vec()),
            tx_index: sea_orm::ActiveValue::Set(self.tx_index as i32),
            coin_delta: sea_orm::ActiveValue::Set(big_int_to_db_vec(self.delta.clone())),
            slot: sea_orm::ActiveValue::Set(u64_to_db_vec(self.slot)),
            block_hash: sea_orm::ActiveValue::Set(self.block_hash.to_vec()),
            ..Default::default()
        }
    }
}
impl From<tx_history::Model> for TransactionInfo {
    fn from(
        tx_history::Model {
            tx_hash,
            tx_index,
            coin_delta,
            slot,
            block_hash,
        }: tx_history::Model,
    ) -> Self {
        TransactionInfo {
            hash: tx_hash.into(),
            block_hash: block_hash.into(),
            slot: u64_from_db_vec(&slot).unwrap(),
            tx_index: tx_index as u16,
            delta: big_int_from_db_vec(&coin_delta),
        }
    }
}
impl OutputFormatter for Vec<TransactionInfo> {
    fn to_table(&self) {
        let mut table = Table::new();
        table.set_header(vec![
            "Slot",
            "Block Hash",
            "Tx Index",
            "Tx Hash",
            "Coin delta",
        ]);

        self.iter().for_each(|tx_info| {
            table.add_row(vec![
                tx_info.slot.to_string(),
                hex::encode(&tx_info.block_hash),
                tx_info.tx_index.to_string(),
                hex::encode(&tx_info.hash),
                tx_info.delta.to_string(),
            ]);
        });

        println!("{table}");
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TxoInfo {
    #[serde(serialize_with = "serialize_bytes_as_hex")]
    pub tx_hash: Bytes,
    pub txo_index: u32,
    #[serde(serialize_with = "serialize_address_as_shelley_bech32")]
    pub address: Bytes,
    pub slot: u64,
    pub coin: u64,
}
impl TxoInfo {
    pub fn as_active_model(&self) -> utxo::ActiveModel {
        entity::utxo::ActiveModel {
            tx_hash: sea_orm::ActiveValue::Set(self.tx_hash.to_vec()),
            txo_index: sea_orm::ActiveValue::Set(self.txo_index as i64),
            address: sea_orm::ActiveValue::Set(self.address.to_vec()),
            slot: sea_orm::ActiveValue::Set(self.slot as i64),
            coin: sea_orm::ActiveValue::Set(u64_to_db_vec(self.coin)),
            ..Default::default()
        }
    }

    pub fn from_parts(
        TxOutput { address, coin, .. }: &TxOutput,
        tx_hash: Bytes,
        txo_index: u32,
        slot: u64,
    ) -> TxoInfo {
        TxoInfo {
            tx_hash: tx_hash,
            txo_index,
            address: address.clone(),
            slot,
            coin: coin.clone(),
        }
    }

    pub fn from_tx_input_output(
        TxOutput { address, coin, .. }: &TxOutput,
        TxInput {
            tx_hash,
            output_index,
            ..
        }: &TxInput,
        slot: u64,
    ) -> TxoInfo {
        TxoInfo {
            tx_hash: tx_hash.clone(),
            txo_index: *output_index,
            address: address.clone(),
            slot,
            coin: coin.clone(),
        }
    }
}
impl From<utxo::Model> for TxoInfo {
    fn from(
        utxo::Model {
            tx_hash,
            txo_index,
            address,
            coin,
            slot,
            ..
        }: utxo::Model,
    ) -> TxoInfo {
        TxoInfo {
            tx_hash: tx_hash.into(),
            txo_index: txo_index.try_into().unwrap(),
            address: address.into(),
            slot: slot.try_into().unwrap(), // TODO Why is slot an i64 here??
            coin: u64_from_db_vec(&coin).unwrap(),
        }
    }
}
impl OutputFormatter for Vec<TxoInfo> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["tx hash", "txo index", "coin"]);

        for utxo in self {
            table.add_row(vec![
                hex::encode(&utxo.tx_hash),
                utxo.txo_index.to_string(),
                utxo.coin.to_string(),
            ]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}

pub fn block_to_model(block: &Block) -> entity::block_history::ActiveModel {
    entity::block_history::ActiveModel {
        hash: sea_orm::ActiveValue::Set(block.header.as_ref().unwrap().hash.to_vec()),
        slot: sea_orm::ActiveValue::Set(u64_to_db_vec(block.header.as_ref().unwrap().slot)),
    }
}

pub fn block_ref_from_recent_point(model: &recent_points::Model) -> BlockRef {
    BlockRef {
        index: model.slot as u64,
        hash: model.block_hash.clone().into(),
    }
}

pub fn block_ref_from_block(block: Block) -> miette::Result<BlockRef> {
    match block.header {
        Some(header) => Ok(BlockRef {
            index: header.slot,
            hash: header.hash,
        }),
        None => bail!("Block cannot be converted to BlockRef because it does not have a header."),
    }
}

pub fn block_ref_from_model(block_history::Model { hash, slot }: block_history::Model) -> BlockRef {
    BlockRef {
        hash: hash.into(),
        index: u64_from_db_vec(&slot).unwrap(),
    }
}

fn serialize_bytes_as_hex<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&hex::encode(&bytes))
}

fn serialize_big_int<S>(big_int: &BigInt, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&big_int.to_string())
}

fn serialize_address_as_shelley_bech32<S>(addr: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let address = match get_shelley_address(&addr.to_vec()) {
        Some(addr) => addr
            .to_bech32()
            .map_err(|err| <S::Error as serde::ser::Error>::custom(format!("{err}"))),
        None => Result::Err(<S::Error as serde::ser::Error>::custom(
            "Could not convert address bytes in TxoInfo to Shelley address",
        )),
    };

    address.and_then(|bech32_addr| serializer.serialize_str(&bech32_addr))
}

fn u64_from_db_vec(db_vec: &Vec<u8>) -> miette::Result<u64> {
    let arr = <[u8; 8]>::try_from(db_vec.as_slice()).into_diagnostic()?;
    Ok(u64::from_le_bytes(arr))
}

fn u64_to_db_vec(num: u64) -> Vec<u8> {
    num.to_le_bytes().into()
}

fn big_int_to_db_vec(num: BigInt) -> Vec<u8> {
    num.to_signed_bytes_le()
}

fn big_int_from_db_vec(db_vec: &Vec<u8>) -> BigInt {
    BigInt::from_signed_bytes_le(&db_vec)
}

pub fn shelley_addr_from_general(addr: Address) -> miette::Result<ShelleyAddress> {
    match addr {
        Address::Shelley(addr) => Ok(addr),
        addr => bail!("Encountered a Byron or Stake address (not yet supported): {addr:?}"),
    }
}
