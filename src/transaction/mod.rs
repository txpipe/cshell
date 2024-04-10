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
    // /// create a new empty transaction in the transaction staging area for the
    // /// specified chain
    // Create(create::Args),
    // /// list transactions which are in the staging area, along with some
    // /// information summary regarding the transaction
    // List(list::Args),
    // /// edit a transaction while still in the staging area
    // Edit(edit::Args),
    // /// remove a transaction from the transaction staging area
    // Delete(delete::Args),
    // /// detailed information on a specific transaction in the staging area
    // Inspect(inspect::Args),
    // /// build/finalize a transaction in the staging area so that it is ready for
    // /// signatures to be attached
    // Build(build::Args),
    // /// sign a transaction using a C-Shell wallet
    // Sign(sign::Args),
    // /// submit a transaction to cardano node
    // Submit(submit::Args),
    // /// export a transaction to json file
    // Export(export::Args),
    // /// show value balance between inputs and outputs
    // Balance(balance::Args),
}

#[instrument("transaction", skip_all)]
pub async fn run(args: Args, _ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        _ => Ok(println!("Not implemented yet. Sorry!")),
        //     Commands::Create(args) => create::run(args, ctx).await,
        //     Commands::List(args) => list::run(args, ctx).await,
        //     Commands::Edit(args) => {
        //         crate::with_tracing();
        //         edit::run(args, ctx).await
        //     }
        //     Commands::Delete(args) => {
        //         crate::with_tracing();
        //         delete::run(args, ctx).await
        //     }
        //     Commands::Inspect(args) => inspect::run(args, ctx).await,
        //     Commands::Build(args) => {
        //         crate::with_tracing();
        //         build::run(args, ctx).await
        //     }
        //     Commands::Sign(args) => sign::run(args, ctx).await,
        //     Commands::Submit(args) => {
        //         crate::with_tracing();
        //         submit::run(args, ctx).await
        //     }
        //     Commands::Export(args) => {
        //         crate::with_tracing();
        //         export::run(args, ctx).await
        //     }
        //     Commands::Balance(args) => balance::run(args, ctx).await,
    }
}
