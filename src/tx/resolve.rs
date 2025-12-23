use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use serde_json::json;
use tracing::instrument;
use tx3_sdk::trp::TxEnvelope;

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for TII file describing transaction invoke interface
    #[arg(long)]
    tii_file: PathBuf,

    /// Json string containing the invoke args for the transaction
    #[arg(long)]
    args_json: Option<String>,

    /// Path for file containing the invoke args for the transaction
    #[arg(long)]
    args_file: Option<PathBuf>,

    /// Which transaction to invoke
    #[arg(long)]
    tx_template: Option<String>,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long)]
    provider: Option<String>,
}

#[instrument("resolve", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let mut invocation = super::common::prepare_invocation(&args.tii_file, args.tx_template)?;

    let all_args = super::common::define_args(
        &mut invocation,
        args.args_json.as_deref(),
        args.args_file.as_deref(),
        ctx,
        provider,
    )?;

    invocation.set_args(all_args);

    let TxEnvelope { tx, hash } = super::common::resolve_tx(invocation, provider).await?;

    let cbor = hex::decode(tx).unwrap();

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
        OutputFormat::Table => println!("{}", hex::encode(&cbor)),
    }

    Ok(())
}
