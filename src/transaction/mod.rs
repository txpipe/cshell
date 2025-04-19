use std::path::PathBuf;

use clap::{Parser, Subcommand};
use jsonrpsee::{
    core::{client::ClientT, params::ObjectParams},
    http_client::HttpClient,
};
use miette::{bail, Context, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use tx3_lang::Protocol;

#[derive(Parser)]
pub struct Args {
    #[arg(long, help = "Path for TX3 file describing transaction")]
    tx3_file: PathBuf,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long, help = "Path for TX3 file describing transaction")]
    provider: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TrpResponse {
    #[serde(with = "hex::serde")]
    tx: Vec<u8>,
}

#[derive(Subcommand)]
enum Commands {}

#[instrument("transaction", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let protocol = Protocol::from_file(args.tx3_file)
        .load()
        .into_diagnostic()
        .context("parsing tx3 file")?;

    let txs: Vec<String> = protocol.txs().map(|x| x.name.to_string()).collect();

    let name = if txs.len() == 1 {
        txs.first().unwrap().clone()
    } else {
        inquire::Select::new("What transaction do you want to build?", txs)
            .prompt()
            .into_diagnostic()?
    };

    let prototx = protocol.new_tx(&name).unwrap();
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
                    .address(true);
                argvalues.insert(key, serde_json::Value::String(address.to_bech32().unwrap()));
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

    let client = HttpClient::builder()
        .build("http://localhost:8000")
        .into_diagnostic()?;

    let mut builder = ObjectParams::new();
    builder
        .insert(
            "tir",
            serde_json::json!({
                "version": "v1alpha1",
                "encoding": "hex",
                "bytecode": hex::encode(prototx.ir_bytes())
            }),
        )
        .unwrap();
    builder.insert("args", argvalues).unwrap();

    let response: TrpResponse = client
        .request("trp.resolve", builder)
        .await
        .into_diagnostic()?;

    let options = ctx
        .store
        .wallets()
        .iter()
        .map(|x| x.name.to_string())
        .collect();
    let wallet = inquire::Select::new(
        "What wallet should be used to sign the transaction?",
        options,
    )
    .prompt()
    .into_diagnostic()?;

    let wallet = ctx
        .store
        .wallets()
        .iter()
        .find(|x| x.name.to_string() == wallet)
        .unwrap();

    let password = inquire::Password::new("Password:")
        .with_help_message("The spending password of your wallet")
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .prompt()
        .into_diagnostic()?;

    let signed = wallet.sign(response.tx, &password)?;
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let txhash = provider.submit(&signed).await?;

    println!("submitted transaction: {}", hex::encode(&signed));
    println!("hash: {}", hex::encode(&txhash));

    Ok(())
}
