use anyhow::{bail, Context, Result};
use clap::Parser;
use tracing::instrument;

use crate::output::OutputFormatter;

#[derive(Parser)]
pub struct Args {
    /// Transaction hash
    #[arg(required = true, help = "Transaction hash")]
    hash: String,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long, help = "Name of the provider to use")]
    provider: Option<String>,
}

#[instrument(skip_all, name = "block")]
pub async fn run(args: Args, ctx: &mut crate::Context) -> Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let hash = hex::decode(args.hash).context("invalid transaction hash")?;

    match provider.fetch_tx(hash).await? {
        Some(v) => {
            v.output(&ctx.output_format);
        }
        None => bail!("transaction hash not found"),
    }

    Ok(())
}
