use clap::Parser;
use miette::bail;
use tracing::instrument;

use super::config::Wallet;
use crate::utils::{Config, ConfigName, OutputFormatter};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to show info for
    #[arg(env = "CSHELL_WALLET")]
    name: String,
}

#[instrument("info", skip_all, fields(name=args.name))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.name)?;
    let wallet = Wallet::load(&ctx.dirs, &name).await?;

    match wallet {
        None => bail!(r#"Wallet named "{}" does not exist."#, &name.raw,),

        Some(wallet) => {
            wallet.output(&ctx.output_format);
            Ok(())
        }
    }
}
