use anyhow::{bail, Context as _, Result};
use inquire::{Confirm, MultiSelect};
use pallas::ledger::addresses::Address;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use tx3_sdk::{
    core::ArgMap,
    tii::{Invocation, ParamMap, ParamType},
    trp::TxEnvelope,
};

use crate::provider::types::Provider;

pub fn load_args(
    invocation: &mut Invocation,
    inline_args: Option<&str>,
    file_args: Option<&Path>,
) -> Result<()> {
    let json_string = match (inline_args, file_args) {
        (Some(inline_args), None) => inline_args.to_string(),
        (None, Some(file_args)) => std::fs::read_to_string(file_args)?,
        (Some(_), Some(_)) => bail!("cannot use both inline and file args"),
        _ => return Ok(()),
    };

    let json_value = serde_json::from_str(&json_string).context("parsing json args string")?;

    let Value::Object(value) = json_value else {
        bail!("json args string must be an object");
    };

    invocation.set_args(value);

    Ok(())
}

fn inquire_transaction(protocol: &tx3_sdk::tii::Protocol) -> Result<String> {
    let keys = protocol
        .txs()
        .keys()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    if keys.len() == 1 {
        return Ok(keys[0].clone());
    }

    let value = inquire::Select::new("Which transaction do you want to build?", keys).prompt()?;

    Ok(value)
}

pub fn prepare_invocation(
    tii_file: &Path,
    tx: Option<&str>,
    profile: Option<&str>,
) -> Result<Invocation> {
    let protocol = tx3_sdk::tii::Protocol::from_file(tii_file).context("parsing tii file")?;

    let tx = match tx {
        Some(x) => x.to_string(),
        None => inquire_transaction(&protocol)?,
    };

    Ok(protocol.invoke(&tx, profile)?)
}

pub fn inquire_missing_args(
    invocation: &mut Invocation,
    ctx: &crate::Context,
    provider: &Provider,
) -> Result<()> {
    let missing: Vec<_> = invocation
        .unspecified_params()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    for (key, value) in missing {
        let text_key = format!("{key}:");

        match value {
            ParamType::Address => {
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

                invocation.set_arg(&key, json!(address.to_string()));
            }
            ParamType::Integer => {
                let value = inquire::Text::new(&text_key)
                    .with_help_message("Enter an integer value")
                    .prompt()?
                    .parse::<u64>()
                    .context("invalid integer value")?;

                invocation.set_arg(&key, json!(value));
            }
            ParamType::UtxoRef => {
                let value = inquire::Text::new(&text_key)
                    .with_help_message("Enter the utxo reference as hash#idx")
                    .prompt()
                    .context("invalid integer value")?;

                invocation.set_arg(&key, json!(value));
            }
            ParamType::Boolean => {
                let value = inquire::Confirm::new(&text_key).prompt()?;

                invocation.set_arg(&key, json!(value));
            }
            ParamType::Bytes => {
                let value = inquire::Text::new(&text_key)
                    .with_help_message("Enter the bytes as hex string")
                    .prompt()?;

                invocation.set_arg(&key, json!(value));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "tx3 arg {key} is of a type not supported via CLI"
                ));
            }
        };
    }

    Ok(())
}

pub fn define_args(
    invocation: &mut Invocation,
    inline_args: Option<&str>,
    file_args: Option<&Path>,
    ctx: &crate::Context,
    provider: &Provider,
) -> Result<()> {
    super::common::load_args(invocation, inline_args, file_args)?;
    super::common::inquire_missing_args(invocation, ctx, provider)?;

    Ok(())
}

pub async fn resolve_tx(invocation: Invocation, provider: &Provider) -> Result<TxEnvelope> {
    let request = invocation.into_resolve_request()?;

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
