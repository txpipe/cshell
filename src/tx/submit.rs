use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::json;
use tracing::instrument;

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    /// Transaction cbor
    cbor: String,

    /// Name of the provider to use. If undefined, will use default
    #[arg(
        long,
        help = "Name of the provider to use. If undefined, will use default"
    )]
    provider: Option<String>,
}

#[instrument("submit", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> Result<()> {
    let cbor = hex::decode(&args.cbor).context("invalid cbor")?;

    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let txhash = provider.submit(&cbor).await?;

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "hash": hex::encode(&txhash)
                }))
                .unwrap()
            );
        }
        OutputFormat::Table => {
            println!("Submitted TX: {}", args.cbor);
            println!("TX Hash: {}", hex::encode(&txhash));
        }
    }

    Ok(())
}
