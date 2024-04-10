use clap::Parser;
use std::fmt::Debug;
use tracing::{info, instrument};
use url::Url;

use super::config::Utxorpc;
use crate::utils::{Config, OutputFormatter};

#[derive(Parser, Debug)]
pub struct Args {
    /// Name of the UTxO RPC configuration (e.g., "preview")
    name: String,
    /// URL of the UTxO RPC endpoint
    url: Url,
    /// Headers to pass to the UTxO RPC endpoint
    #[arg(short('H'), long, value_parser = crate::utils::parse_key_value, value_name = "KEY:VALUE")]
    headers: Vec<(String, String)>,
}

#[instrument("create", skip_all, fields(name=args.name))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let cfg = Utxorpc::new(args.name, args.url, args.headers)?;

    cfg.save(&ctx.dirs, false).await?;

    info!(u5c_name = &cfg.name.raw, "UTxO RPC configured");
    println!("Created the following UTxO RPC configuration:",);
    cfg.output(&ctx.output_format);
    Ok(())
}
