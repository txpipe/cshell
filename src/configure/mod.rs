use clap::{Parser, Subcommand};
use tracing::instrument;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    CommandA,
    CommandB,
}

#[instrument("configure", skip_all)]
pub async fn run(args: Args, _ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::CommandA => Ok(println!("Not implemented yet. Sorry!")),
        Commands::CommandB => Ok(println!("Not implemented yet. Sorry!")),
    }
}
