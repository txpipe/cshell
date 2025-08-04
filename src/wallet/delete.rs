use clap::Parser;
use tracing::{info, instrument};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to delete
    name: String,
}

#[instrument("delete", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> anyhow::Result<()> {
    let wallet = ctx.store.find_wallet(&args.name);
    match wallet {
        Some(wallet) => {
            info!("Removing wallet...");
            ctx.store.remove_wallet(wallet.clone())
        }
        None => {
            info!("Wallet not found.");
            Ok(())
        }
    }
}
