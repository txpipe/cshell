use clap::Parser;
use miette::bail;
use tracing::instrument;

use crate::utils::{Config, ConfigName, OutputFormatter};

use super::config::Utxorpc;

#[derive(Parser)]
pub struct Args {
    /// Name of the configuration
    name: String,
}

#[instrument("info", skip_all, fields(name=args.name))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.name.clone())?;
    let cfg: Option<Utxorpc> = Utxorpc::load(&ctx.dirs, &name).await?;

    match cfg {
        None => bail!(r#"Configuration named "{}" does not exist."#, &args.name,),
        Some(cfg) => cfg.output(&ctx.output_format),
    }

    Ok(())
}
