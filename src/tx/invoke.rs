use clap::Parser;
use miette::{bail, Context as _, IntoDiagnostic};
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::instrument;
use tx3_sdk::trp::TxEnvelope;

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for TX3 file describing transaction
    #[arg(long)]
    tx3_file: PathBuf,

    /// Args for TX3 file describing transaction
    #[arg(long)]
    tx3_args_json: Option<String>,

    /// Template for TX3 file
    #[arg(long)]
    tx3_template: Option<String>,

    /// Wallets that will sign the transaction
    #[arg(long)]
    signers: Vec<String>,

    /// Skip submitting
    #[arg(long)]
    skip_submit: bool,

    /// Allow signing with unsafe wallets
    #[arg(long)]
    r#unsafe: bool,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long)]
    provider: Option<String>,
}

#[instrument("invoke", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let prototx = super::common::load_prototx(&args.tx3_file, args.tx3_template, ctx)?;

    let argvalues = match args.tx3_args_json {
        Some(args) => {
            let json_value = serde_json::from_str(&args)
                .into_diagnostic()
                .context("invalid tx3-args-json")?;

            let Value::Object(value) = json_value else {
                bail!("tx3-args-json must be an object");
            };

            value
        }
        None => super::common::inquire_args(&prototx, ctx, provider)?,
    };

    let TxEnvelope { tx, hash } = super::common::resolve_tx(&prototx, argvalues, provider).await?;

    let cbor = hex::decode(tx).unwrap();

    let cbor = super::common::sign_tx(&cbor, ctx, args.signers, args.r#unsafe).await?;

    if !args.skip_submit {
        provider.submit(&cbor).await?;
    }

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "hash": hex::encode(&hash),
                    "cbor": hex::encode(&cbor),
                }))
                .unwrap()
            );
        }

        OutputFormat::Table => {
            println!("Tx Hash: {}", hex::encode(&hash));
            println!("Tx CBOR: {}", hex::encode(&cbor));
        }
    }

    Ok(())
}
