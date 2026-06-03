use anyhow::{bail, Context as _, Result};
use inquire::{Confirm, MultiSelect};
use pallas::ledger::addresses::Address;
use serde_json::{json, Value};
use std::path::Path;

use tx3_sdk::{
    tii::{Invocation, ParamType, Protocol},
    trp::TxEnvelope,
};

use crate::provider::types::Provider;

const NAMESPACED_BYTES_REF: &str = "https://tx3.land/specs/v1beta0/tii#/$defs/Bytes";
const NAMESPACED_ADDRESS_REF: &str = "https://tx3.land/specs/v1beta0/tii#/$defs/Address";
const NAMESPACED_UTXO_REF: &str = "https://tx3.land/specs/v1beta0/tii#/$defs/UtxoRef";
const CORE_BYTES_REF: &str = "https://tx3.land/specs/v1beta0/core#Bytes";
const CORE_ADDRESS_REF: &str = "https://tx3.land/specs/v1beta0/core#Address";
const CORE_UTXO_REF: &str = "https://tx3.land/specs/v1beta0/core#UtxoRef";

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

fn normalize_tii_ref(reference: &str) -> Option<&'static str> {
    match reference {
        NAMESPACED_BYTES_REF => Some(CORE_BYTES_REF),
        NAMESPACED_ADDRESS_REF => Some(CORE_ADDRESS_REF),
        NAMESPACED_UTXO_REF => Some(CORE_UTXO_REF),
        _ => None,
    }
}

fn normalize_tii_json_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(normalized) = normalize_tii_ref(reference) {
                    map.insert("$ref".to_string(), Value::String(normalized.to_string()));
                }
            }

            for nested in map.values_mut() {
                normalize_tii_json_refs(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_tii_json_refs(item);
            }
        }
        _ => {}
    }
}

fn load_protocol_from_tii_file(tii_file: &Path) -> Result<Protocol> {
    let raw = std::fs::read_to_string(tii_file)?;
    let mut json: Value = serde_json::from_str(&raw)?;
    normalize_tii_json_refs(&mut json);
    Protocol::from_json(json).map_err(Into::into)
}

pub fn prepare_invocation(
    tii_file: &Path,
    tx: Option<&str>,
    profile: Option<&str>,
) -> Result<Invocation> {
    let protocol = load_protocol_from_tii_file(tii_file).context("parsing tii file")?;

    let tx = match tx {
        Some(x) => x.to_string(),
        None => inquire_transaction(&protocol)?,
    };

    Ok(protocol.invoke(&tx, profile)?)
}

fn inquire_custom_address(param_key: &str) -> Result<Address> {
    let value = inquire::Text::new(&format!("{param_key}:"))
        .with_help_message("Enter a bech32 address")
        .prompt()?;

    Ok(Address::from_bech32(&value).context("invalid bech32 address")?)
}

fn inquire_address(ctx: &crate::Context, provider: &Provider, param_key: &str) -> Result<Address> {
    let custom_address = String::from("custom address");

    let mut options = ctx
        .store
        .wallets()
        .iter()
        .map(|x| x.name.to_string())
        .collect::<Vec<String>>();

    if options.is_empty() {
        return inquire_custom_address(param_key);
    }

    options.push(custom_address.clone());

    let wallet = inquire::Select::new(&format!("{param_key}:"), options).prompt()?;

    if wallet.eq(&custom_address) {
        Ok(inquire_custom_address(param_key)?)
    } else {
        let value = ctx
            .store
            .wallets()
            .iter()
            .find(|x| x.name.to_string() == wallet)
            .unwrap()
            .address(provider.is_testnet());

        Ok(value)
    }
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
                let value = inquire_address(ctx, provider, &key)?;
                invocation.set_arg(&key, json!(value.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn write_temp_tii_file(name: &str, content: serde_json::Value) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cshell-{name}-{suffix}.tii"));
        fs::write(&path, serde_json::to_vec_pretty(&content).unwrap()).unwrap();
        path
    }

    fn sample_tii(bytes_ref: &str, address_ref: &str, utxo_ref: &str) -> serde_json::Value {
        json!({
            "tii": { "version": "v1beta0" },
            "protocol": {
                "name": "repro",
                "scope": "eryxcoop",
                "version": "1.0.0"
            },
            "parties": {
                "user": {}
            },
            "transactions": {
                "demo": {
                    "params": {
                        "type": "object",
                        "properties": {
                            "payload": { "$ref": bytes_ref },
                            "recipient": { "$ref": address_ref },
                            "source_utxo": { "$ref": utxo_ref },
                            "quantity": { "type": "integer" }
                        },
                        "required": ["payload", "recipient", "source_utxo", "quantity"]
                    },
                    "tir": {
                        "content": "",
                        "encoding": "hex",
                        "version": "v1beta0"
                    }
                }
            },
            "profiles": {}
        })
    }

    #[test]
    fn prepare_invocation_accepts_legacy_core_refs() {
        let path = write_temp_tii_file(
            "legacy-core",
            sample_tii(CORE_BYTES_REF, CORE_ADDRESS_REF, CORE_UTXO_REF),
        );

        let result = prepare_invocation(&path, Some("demo"), None);

        let _ = fs::remove_file(path);
        assert!(result.is_ok());
    }

    #[test]
    fn prepare_invocation_accepts_namespaced_tii_refs() {
        let path = write_temp_tii_file(
            "namespaced",
            sample_tii(
                NAMESPACED_BYTES_REF,
                NAMESPACED_ADDRESS_REF,
                NAMESPACED_UTXO_REF,
            ),
        );

        let result = prepare_invocation(&path, Some("demo"), None);

        let _ = fs::remove_file(path);
        assert!(result.is_ok());
    }

    #[test]
    fn prepare_invocation_still_rejects_unknown_refs() {
        let path = write_temp_tii_file(
            "unknown-ref",
            sample_tii(
                CORE_BYTES_REF,
                CORE_ADDRESS_REF,
                "https://tx3.land/specs/v1beta0/tii#/$defs/Unknown",
            ),
        );

        let result = prepare_invocation(&path, Some("demo"), None);

        let _ = fs::remove_file(path);
        let error = result.expect_err("unknown refs should still fail");
        assert!(error.to_string().contains("invalid param type"));
    }
}
