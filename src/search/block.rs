use anyhow::{bail, Result};
use clap::Parser;
use regex::Regex;
use tracing::instrument;

use crate::output::OutputFormatter;

#[derive(Parser)]
pub struct Args {
    /// List of hash,index
    #[arg(required = true, help = "List of hash,index to fetch block")]
    refs: Vec<String>,

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

    let ref_regex = Regex::new(r"(.+),(\d+)")?;

    let refs = args
        .refs
        .iter()
        .map(|r| {
            let captures = ref_regex
                .captures(r)
                .ok_or_else(|| anyhow::Error::msg(format!("Invalid reference format: {r}")))?;

            let hash_str = captures.get(1).unwrap().as_str();
            let index = captures.get(2).unwrap().as_str().parse::<u64>()?;

            let decoded_hash = hex::decode(hash_str)?;

            Ok((decoded_hash, index))
        })
        .collect::<Result<Vec<(Vec<u8>, u64)>>>()?;

    let blocks = provider.fetch_block(refs).await?;
    blocks.output(&ctx.output_format);

    Ok(())
}
