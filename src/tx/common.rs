use anyhow::{bail, Context as _, Result};
use inquire::{Confirm, MultiSelect};
use pallas::ledger::addresses::Address;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};
use tx3_lang::{ArgValue, ProtoTx, Protocol, UtxoRef};
use tx3_sdk::trp::{self, TxEnvelope};

use crate::provider::types::Provider;

pub fn load_args(
    inline_args: Option<&str>,
    file_args: Option<&Path>,
    params: &BTreeMap<String, tx3_lang::ir::Type>,
) -> Result<HashMap<String, ArgValue>> {
    let json_string = match (inline_args, file_args) {
        (Some(inline_args), None) => inline_args.to_string(),
        (None, Some(file_args)) => std::fs::read_to_string(file_args)?,
        (Some(_), Some(_)) => bail!("cannot use both inline and file args"),
        _ => return Ok(HashMap::new()),
    };

    let json_value = serde_json::from_str(&json_string).context("parsing json args string")?;

    let Value::Object(mut value) = json_value else {
        bail!("json args string must be an object");
    };

    let mut args = HashMap::new();

    for (key, ty) in params {
        if let Some(value) = value.remove(key) {
            let arg_value = tx3_sdk::trp::args::from_json(value, ty)?;
            args.insert(key.clone(), arg_value);
        }
    }

    Ok(args)
}

pub fn load_prototx(tx3_file: &Path, tx3_template: Option<String>) -> Result<ProtoTx> {
    let protocol = Protocol::from_file(tx3_file)
        .load()
        .context("parsing tx3 file")?;

    let txs: Vec<String> = protocol.txs().map(|x| x.name.value.to_string()).collect();

    let template = match tx3_template {
        Some(template) => template,
        None => {
            let template = if txs.len() == 1 {
                txs.first().unwrap().clone()
            } else {
                inquire::Select::new("What transaction do you want to build?", txs).prompt()?
            };
            template
        }
    };

    Ok(protocol.new_tx(&template)?)
}

pub fn inquire_args(
    params: &BTreeMap<String, tx3_lang::ir::Type>,
    ctx: &crate::Context,
    provider: &Provider,
) -> Result<HashMap<String, ArgValue>> {
    let mut argvalues = HashMap::new();

    for (key, value) in params {
        let text_key = format!("{key}:");

        match value {
            tx3_lang::ir::Type::Address => {
                let custom_address = String::from("custom address");
                let mut options = ctx
                    .store
                    .wallets()
                    .iter()
                    .map(|x| x.name.to_string())
                    .collect::<Vec<String>>();

                options.push(custom_address.clone());

                let wallet = inquire::Select::new(&text_key, options).prompt()?;

                let address = if wallet.eq(&custom_address) {
                    let value = inquire::Text::new("address:")
                        .with_help_message("Enter a bech32 address")
                        .prompt()?;

                    Address::from_bech32(&value).context("invalid bech32 address")?
                } else {
                    ctx.store
                        .wallets()
                        .iter()
                        .find(|x| x.name.to_string() == wallet)
                        .unwrap()
                        .address(provider.is_testnet())
                };

                argvalues.insert(key.clone(), trp::ArgValue::Address(address.to_vec()));
            }
            tx3_lang::ir::Type::Int => {
                let value = inquire::Text::new(&text_key)
                    .with_help_message("Enter an integer value")
                    .prompt()?
                    .parse::<u64>()
                    .context("invalid integer value")?;

                argvalues.insert(key.clone(), trp::ArgValue::Int(value.into()));
            }
            tx3_lang::ir::Type::UtxoRef => {
                let value = inquire::Text::new(&text_key)
                    .with_help_message("Enter the utxo reference as hash#idx")
                    .prompt()
                    .context("invalid integer value")?;

                let (hash, idx) = value
                    .split_once('#')
                    .ok_or_else(|| anyhow::anyhow!("expected format: <hash>#<index>"))?;

                let hash = hex::decode(hash).context("invalid hex value for hash")?;

                let idx: u32 = idx.parse().context("invalid integer value for index")?;

                let utxo_ref = UtxoRef::new(hash.as_slice(), idx);
                argvalues.insert(key.clone(), trp::ArgValue::UtxoRef(utxo_ref));
            }
            tx3_lang::ir::Type::Bool => {
                let value = inquire::Confirm::new(&text_key).prompt()?;

                argvalues.insert(key.clone(), trp::ArgValue::Bool(value));
            }
            tx3_lang::ir::Type::Bytes => {
                let value = inquire::Text::new(&text_key)
                    .with_help_message("Enter the bytes as hex string")
                    .prompt()?;

                let value = hex::decode(value).context("invalid hex value")?;

                argvalues.insert(key.clone(), trp::ArgValue::Bytes(value));
            }

            tx3_lang::ir::Type::Undefined => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is of type Undefined, not supported yet"
                ));
            }
            tx3_lang::ir::Type::Unit => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is of type Unit, not supported yet",
                ));
            }
            tx3_lang::ir::Type::Utxo => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is of type Utxo, not supported yet"
                ));
            }
            tx3_lang::ir::Type::AnyAsset => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is of type AnyAsset, not supported yet"
                ));
            }
            tx3_lang::ir::Type::List => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is of type List, not supported yet",
                ));
            }
            tx3_lang::ir::Type::Custom(x) => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is a custom type {x}, not supported yet"
                ));
            }
        };
    }

    Ok(argvalues)
}

pub fn define_args(
    params: &BTreeMap<String, tx3_lang::ir::Type>,
    inline_args: Option<&str>,
    file_args: Option<&Path>,
    ctx: &crate::Context,
    provider: &Provider,
) -> Result<HashMap<String, ArgValue>> {
    let mut remaining_params = params.clone();

    let mut loaded_args =
        super::common::load_args(inline_args, file_args, &remaining_params)?;

    // remove from the remaining params the args we already managed to load from the
    // file or json
    for key in loaded_args.keys() {
        remaining_params.remove(key);
    }

    // inquire the user for the remaining args
    let inquired_args = super::common::inquire_args(&remaining_params, ctx, provider)?;

    loaded_args.extend(inquired_args);

    Ok(loaded_args)
}

pub async fn resolve_tx(
    prototx: &ProtoTx,
    args: HashMap<String, ArgValue>,
    provider: &Provider,
) -> Result<TxEnvelope> {
    let request = tx3_sdk::trp::ProtoTxRequest {
        tir: tx3_sdk::trp::TirInfo {
            version: tx3_lang::ir::IR_VERSION.to_string(),
            encoding: "hex".to_string(),
            bytecode: hex::encode(prototx.ir_bytes()),
        },
        args,
    };

    provider.trp_resolve(request).await
}

pub async fn sign_tx(
    cbor: &[u8],
    ctx: &crate::Context,
    signers: Vec<String>,
    allow_unsafe: bool,
) -> Result<Vec<u8>> {
    let mut cbor = cbor.to_vec();

    let signers = if signers.is_empty() {
        let wallet_names: Vec<String> = ctx
            .store
            .wallets()
            .iter()
            .map(|wallet| wallet.name.to_string())
            .collect();

        MultiSelect::new(
            "What wallet should be used to sign the transaction?",
            wallet_names,
        )
        .prompt()
        .unwrap_or_default()
    } else {
        signers.clone()
    };

    let wallets = signers
        .iter()
        .map(|signer| {
            let wallet = ctx
                .store
                .wallets()
                .iter()
                .find(|wallet| wallet.name.to_string().eq(signer));

            let Some(wallet) = wallet else {
                bail!("invalid signer wallet '{signer}'")
            };

            if wallet.is_unsafe && !allow_unsafe {
                let confirm = Confirm::new(&format!("wallet '{signer}' is unsafe, confirm sign?"))
                    .with_default(false)
                    .prompt()
                    .unwrap_or_default();

                if !confirm {
                    bail!(
                        "wallet '{signer}' is unsafe, use the param --unsafe to allow unsafe signatures"
                    )
                }
            }

            Ok(wallet)
        })
        .collect::<Result<Vec<_>, _>>()?;

    for wallet in wallets {
        let password = match wallet.is_unsafe {
            true => None,
            false => Some(
                inquire::Password::new("Password:")
                    .with_help_message(&format!(
                        "The spending password for '{}' wallet:",
                        wallet.name
                    ))
                    .with_display_mode(inquire::PasswordDisplayMode::Masked)
                    .prompt()?,
            ),
        };

        cbor = wallet.sign(cbor, &password)?;
    }

    Ok(cbor)
}
