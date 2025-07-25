use std::path::PathBuf;

use clap::Parser;
use inquire::{Confirm, MultiSelect};
use jsonrpsee::core::params::ObjectParams;
use miette::{bail, Context, IntoDiagnostic};
use pallas::ledger::addresses::Address;
use serde_json::json;
use tracing::instrument;
use tx3_lang::{Protocol, UtxoRef};
use tx3_sdk::trp::{self, ArgValue};

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    #[arg(long, help = "Path for TX3 file describing transaction")]
    tx3_file: PathBuf,

    #[arg(long, help = "Args for TX3 file describing transaction")]
    tx3_args_json: Option<String>,

    #[arg(long, help = "Template for TX3 file")]
    tx3_template: Option<String>,

    #[arg(long, help = "Wallets that will sign the transaction")]
    signer: Vec<String>,

    /// Allow sign using unsafe wallets
    #[arg(long, help = "Allow unsafe wallet signatures")]
    r#unsafe: bool,

    /// Name of the provider to use. If undefined, will use default
    #[arg(
        long,
        help = "Name of the provider to use. If undefined, will use default"
    )]
    provider: Option<String>,
}

#[instrument("new", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let protocol = Protocol::from_file(args.tx3_file)
        .load()
        .into_diagnostic()
        .context("parsing tx3 file")?;

    let txs: Vec<String> = protocol.txs().map(|x| x.name.value.to_string()).collect();

    let template = match args.tx3_template {
        Some(template) => template,
        None => {
            let template = if txs.len() == 1 {
                txs.first().unwrap().clone()
            } else {
                inquire::Select::new("What transaction do you want to build?", txs)
                    .prompt()
                    .into_diagnostic()?
            };
            template
        }
    };

    let prototx = protocol.new_tx(&template).unwrap();

    let argvalues = match args.tx3_args_json {
        Some(args) => {
            let json_value = serde_json::from_str(&args)
                .into_diagnostic()
                .context("invalid tx3-args-json")?;

            let serde_json::Value::Object(value) = json_value else {
                bail!("tx3-args-json must be an object");
            };

            value
        }
        None => {
            let params = prototx.find_params();
            let mut argvalues = serde_json::Map::new();
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

                        let wallet = inquire::Select::new(&text_key, options)
                            .prompt()
                            .into_diagnostic()?;

                        let address = if wallet.eq(&custom_address) {
                            let value = inquire::Text::new("address:")
                                .with_help_message("Enter a bech32 address")
                                .prompt()
                                .into_diagnostic()?;

                            Address::from_bech32(&value)
                                .into_diagnostic()
                                .context("invalid bech32 address")?
                        } else {
                            ctx.store
                                .wallets()
                                .iter()
                                .find(|x| x.name.to_string() == wallet)
                                .unwrap()
                                .address(provider.is_testnet())
                        };

                        argvalues
                            .insert(key, trp::args::to_json(ArgValue::Address(address.to_vec())));
                    }
                    tx3_lang::ir::Type::Int => {
                        let value = inquire::Text::new(&text_key)
                            .with_help_message("Enter an integer value")
                            .prompt()
                            .into_diagnostic()?
                            .parse::<u64>()
                            .into_diagnostic()
                            .context("invalid integer value")?;

                        argvalues.insert(key, trp::args::to_json(ArgValue::Int(value.into())));
                    }
                    tx3_lang::ir::Type::UtxoRef => {
                        let value = inquire::Text::new(&text_key)
                            .with_help_message("Enter the utxo reference as hash#idx")
                            .prompt()
                            .into_diagnostic()
                            .context("invalid integer value")?;

                        let (hash, idx) = value
                            .split_once('#')
                            .ok_or_else(|| miette::miette!("expected format: <hash>#<index>"))?;

                        let hash = hex::decode(hash)
                            .into_diagnostic()
                            .context("invalid hex value for hash")?;

                        let idx: u32 = idx
                            .parse()
                            .into_diagnostic()
                            .context("invalid integer value for index")?;

                        let utxo_ref = UtxoRef::new(hash.as_slice(), idx);
                        argvalues.insert(key, trp::args::to_json(ArgValue::UtxoRef(utxo_ref)));
                    }
                    tx3_lang::ir::Type::Bool => {
                        let value = inquire::Confirm::new(&text_key)
                            .prompt()
                            .into_diagnostic()?;

                        argvalues.insert(key, trp::args::to_json(ArgValue::Bool(value)));
                    }
                    tx3_lang::ir::Type::Bytes => {
                        let value = inquire::Text::new(&text_key)
                            .with_help_message("Enter the bytes as hex string")
                            .prompt()
                            .into_diagnostic()?;

                        let value = hex::decode(value)
                            .into_diagnostic()
                            .context("invalid hex value")?;

                        argvalues.insert(key, trp::args::to_json(ArgValue::Bytes(value)));
                    }

                    tx3_lang::ir::Type::Undefined => {
                        return Err(miette::miette!(
                            "tx3 arg {key} is of type Undefined, not supported yet"
                        ));
                    }
                    tx3_lang::ir::Type::Unit => {
                        return Err(miette::miette!(
                            "tx3 arg {key} is of type Unit, not supported yet",
                        ));
                    }
                    tx3_lang::ir::Type::Utxo => {
                        return Err(miette::miette!(
                            "tx3 arg {key} is of type Utxo, not supported yet"
                        ));
                    }
                    tx3_lang::ir::Type::AnyAsset => {
                        return Err(miette::miette!(
                            "tx3 arg {key} is of type AnyAsset, not supported yet"
                        ));
                    }
                    tx3_lang::ir::Type::List => {
                        return Err(miette::miette!(
                            "tx3 arg {key} is of type List, not supported yet",
                        ));
                    }
                    tx3_lang::ir::Type::Custom(x) => {
                        return Err(miette::miette!(
                            "tx3 arg {key} is a custom type {x}, not supported yet"
                        ));
                    }
                };
            }
            argvalues
        }
    };

    let mut builder = ObjectParams::new();
    builder
        .insert(
            "tir",
            serde_json::json!({
                "version": tx3_lang::ir::IR_VERSION.to_string(),
                "encoding": "hex",
                "bytecode": hex::encode(prototx.ir_bytes())
            }),
        )
        .unwrap();
    builder.insert("args", argvalues).unwrap();

    let signers = if args.signer.is_empty() {
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
        args.signer.clone()
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

            if wallet.is_unsafe && !args.r#unsafe {
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

    let response = provider.trp_resolve(&builder).await?;
    let mut cbor = response.tx;

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
                    .prompt()
                    .into_diagnostic()?,
            ),
        };

        cbor = wallet.sign(cbor, &password)?;
    }

    let txhash = provider.submit(&cbor).await?;

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "hash": hex::encode(&txhash),
                    "cbor": hex::encode(&cbor),
                }))
                .unwrap()
            );
        }

        OutputFormat::Table => {
            println!("TX Hash: {}", hex::encode(&txhash));
            println!("Submitted TX: {}", hex::encode(&cbor));
        }
    }

    Ok(())
}
