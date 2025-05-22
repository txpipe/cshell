use std::path::PathBuf;

use clap::{Parser, Subcommand};
use jsonrpsee::core::params::ObjectParams;
use miette::{bail, Context, IntoDiagnostic};
use tracing::instrument;
use tx3_lang::Protocol;

mod resolve;
mod sign;
mod submit;

#[derive(Parser)]
pub struct Args {
    #[arg(long, help = "Path for TX3 file describing transaction")]
    tx3_file: Option<PathBuf>,

    #[arg(long, help = "Args for TX3 file describing transaction")]
    tx3_args_json: Option<String>,

    #[arg(long, help = "Template for TX3 file")]
    tx3_template: Option<String>,

    #[arg(long, help = "Wallet that will sign the transaction")]
    signer: Option<String>,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long, help = "Path for TX3 file describing transaction")]
    provider: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Resolve a transaction
    Resolve(resolve::Args),

    /// Sign a transaction cbor
    Sign(sign::Args),

    /// Submit a transaction cbor
    Submit(submit::Args),
}

#[instrument("transaction", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Some(command) => match command {
            Commands::Sign(args) => sign::run(args, ctx).await?,
            Commands::Submit(args) => submit::run(args, ctx).await?,
            Commands::Resolve(args) => resolve::run(args, ctx).await?,
        },
        None => {
            let provider = match args.provider {
                Some(name) => ctx.store.find_provider(&name),
                None => ctx.store.default_provider(),
            };

            let Some(provider) = provider else {
                bail!("Provider not found")
            };

            let Some(tx3_file) = args.tx3_file else {
                bail!("Tx3 file not provided")
            };

            let protocol = Protocol::from_file(tx3_file)
                .load()
                .into_diagnostic()
                .context("parsing tx3 file")?;

            let txs: Vec<String> = protocol.txs().map(|x| x.name.to_string()).collect();

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
                        .context("invalid tx3-args")?;

                    let serde_json::Value::Object(value) = json_value else {
                        bail!("tx3-args must be an object");
                    };

                    value
                }
                None => {
                    let params = prototx.find_params();
                    let mut argvalues = serde_json::Map::new();
                    for (key, value) in params {
                        match value {
                            tx3_lang::ir::Type::Address => {
                                let options = ctx
                                    .store
                                    .wallets()
                                    .iter()
                                    .map(|x| x.name.to_string())
                                    .collect();
                                let wallet = inquire::Select::new(&format!("{}: ", &key), options)
                                    .prompt()
                                    .into_diagnostic()?;
                                let address = ctx
                                    .store
                                    .wallets()
                                    .iter()
                                    .find(|x| x.name.to_string() == wallet)
                                    .unwrap()
                                    .address(provider.is_testnet());
                                argvalues.insert(
                                    key,
                                    serde_json::Value::String(address.to_bech32().unwrap()),
                                );
                            }
                            tx3_lang::ir::Type::Int => {
                                let value = inquire::Text::new(&format!("{}: ", &key))
                                    .prompt()
                                    .into_diagnostic()?
                                    .parse::<u64>()
                                    .into_diagnostic()
                                    .context("invalid integer value")?;
                                argvalues.insert(key, serde_json::Value::Number(value.into()));
                            }
                            _ => todo!(),
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

            let response = provider.trp_resolve(&builder).await?;
            let signer = match args.signer {
                Some(signer) => signer,
                None => {
                    let options = ctx
                        .store
                        .wallets()
                        .iter()
                        .map(|x| x.name.to_string())
                        .collect();
                    inquire::Select::new(
                        "What wallet should be used to sign the transaction?",
                        options,
                    )
                    .prompt()
                    .into_diagnostic()?
                }
            };

            let wallet = ctx
                .store
                .wallets()
                .iter()
                .find(|x| x.name.to_string() == signer);

            let Some(wallet) = wallet else {
                bail!("invalid signer wallet")
            };

            let password = match wallet.is_unsafe {
                true => None,
                false => Some(
                    inquire::Password::new("Password:")
                        .with_help_message("The spending password of your wallet")
                        .with_display_mode(inquire::PasswordDisplayMode::Masked)
                        .prompt()
                        .into_diagnostic()?,
                ),
            };

            let signed = wallet.sign(response.tx, &password)?;
            let txhash = provider.submit(&signed).await?;

            println!("Submitted TX: {}", hex::encode(&signed));
            println!("TX Hash: {}", hex::encode(&txhash));
        }
    };

    Ok(())
}
