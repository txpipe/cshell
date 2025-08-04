use clap::Parser;
use tracing::{info, instrument};

#[derive(Parser)]
pub struct Args {
    /// Name of the provider to delete
    name: String,
}

#[instrument("delete", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> anyhow::Result<()> {
    let provider = ctx.store.find_provider(&args.name);
    match provider {
        Some(provider) => {
            info!("Removing provider...");
            ctx.store.remove_provider(provider.clone())
        }
        None => {
            info!("Provider not found.");
            Ok(())
        }
    }
}
