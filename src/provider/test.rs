use clap::Parser;
use miette::bail;

#[derive(Parser)]
pub struct Args {
    /// Name of the provider to test connection with. If undefined will use default.
    #[arg(long)]
    name: Option<String>,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let provider = match args.name {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    match provider {
        Some(provider) => provider.test().await,
        None => bail!("Provider not found, and no default provider configured."),
    }
}
