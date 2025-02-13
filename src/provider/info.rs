use clap::Parser;
use miette::bail;
use tracing::instrument;

use crate::output::OutputFormatter;

#[derive(Parser)]
pub struct Args {
    /// Name of the provider to show info for. If undefined, will use default
    name: Option<String>,
}

#[instrument("info", skip_all, fields(name=args.name))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let provider = match args.name {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    match provider {
        Some(provider) => {
            provider.output(&ctx.output_format);
            Ok(())
        }
        None => bail!("Wallet not found."),
    }
}
