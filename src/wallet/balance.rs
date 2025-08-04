use anyhow::bail;
use clap::Parser;

use crate::output::OutputFormatter;

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to show the balance of. If undefined, will use
    /// default
    name: Option<String>,

    /// Name of the provider to use. If undefined, will use default
    provider: Option<String>,

    /// Whether to include details of all UTxOs or aggregated data.
    #[arg(long, action)]
    detail: bool,
}

pub async fn run(args: Args, ctx: &crate::Context) -> anyhow::Result<()> {
    let wallet = match args.name {
        Some(name) => ctx.store.find_wallet(&name),
        None => ctx.store.default_wallet(),
    };

    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    match (wallet, provider) {
        (Some(wallet), Some(provider)) => {
            if args.detail {
                let balance = provider
                    .get_detailed_balance(&wallet.address(provider.is_testnet()))
                    .await?;
                balance.output(&ctx.output_format);
            } else {
                let balance = provider
                    .get_balance(&wallet.address(provider.is_testnet()))
                    .await?;
                balance.output(&ctx.output_format);
            }

            Ok(())
        }
        (None, Some(_)) => bail!("Wallet not found."),
        (Some(_), None) => bail!("Provider not found."),
        (None, None) => bail!("Wallet and provider not found."),
    }
}
