use clap::Parser;
use miette::{bail, IntoDiagnostic};
use tracing::instrument;

use crate::utils::{Config, ConfigName};

use super::config::Utxorpc;

#[derive(Parser)]
pub struct Args {
    /// Name of the UTxO RPC configuration to delete
    name: String,
    /// Do not fail if config does not exist (default: false)
    #[arg(short, long)]
    quiet: bool,
}

#[instrument("delete", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.name.clone())?;
    let cfg_dir_path = Utxorpc::dir_path_of(&ctx.dirs, &name);
    let exists = cfg_dir_path.exists();

    match (exists, args.quiet) {
        (false, false) => bail!(r#"UTxO RPC config named "{}" does not exist."#, &args.name,),
        (false, true) => Ok(()),
        (true, _) => tokio::fs::remove_dir_all(&cfg_dir_path)
            .await
            .into_diagnostic(),
    }
}
