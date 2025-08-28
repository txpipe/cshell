use anyhow::{bail, Result};
use clap::Parser;
use serde_json::json;
use std::path::PathBuf;
use tracing::instrument;
use tx3_sdk::trp::TxEnvelope;

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for Tx3 file describing transaction
    #[arg(long)]
    tx3_file: PathBuf,

    /// Json string containing args for the Tx3 transaction
    #[arg(long)]
    tx3_args_json: Option<String>,

    /// Path for file containing args for the Tx3 transaction
    #[arg(long)]
    tx3_args_file: Option<PathBuf>,

    /// Template for Tx3 file
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
pub async fn run(args: Args, ctx: &crate::Context) -> Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let prototx = super::common::load_prototx(&args.tx3_file, args.tx3_template)?;

    let tx_args = super::common::define_args(
        &prototx.find_params(),
        args.tx3_args_json.as_deref(),
        args.tx3_args_file.as_deref(),
        ctx,
        provider,
    )?;

    let TxEnvelope { tx, hash } = super::common::resolve_tx(&prototx, tx_args, provider).await?;

    let cbor = hex::decode(tx).unwrap();

    let cbor = super::common::sign_tx(&cbor, ctx, args.signers, args.r#unsafe).await?;

    if !args.skip_submit {
        provider
            .trp_submit(TxEnvelope {
                tx: hex::encode(&cbor),
                hash: hash.clone(),
            })
            .await?;
    }

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "hash": hash,
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
