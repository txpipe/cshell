use clap::Parser;
use miette::bail;
use tracing::instrument;

use crate::output::OutputFormatter;

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to show info for. If undefined, will use default
    #[arg(long)]
    name: Option<String>,
}

#[instrument("info", skip_all, fields(name=args.name))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let wallet = match args.name {
        Some(name) => ctx.store.find_wallet(&name),
        None => ctx.store.default_wallet(),
    };

    match wallet {
        Some(wallet) => {
            wallet.output(&ctx.output_format);
            Ok(())
        }
        None => bail!("Wallet not found."),
    }
}
