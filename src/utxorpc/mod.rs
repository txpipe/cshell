use clap::{Parser, Subcommand};
use miette::{bail, IntoDiagnostic};
use tracing::{info, instrument};
use url::Url;

use crate::{
    utils::{Config, ConfigName, OutputFormatter},
    utxorpc::config::Utxorpc,
};

pub mod config;
pub mod dump;
pub mod follow_tip;
pub mod get_block;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new UTxO RPC configuration
    Create(CreateArgs),
    /// Get info about a UTxO configuration
    Info(InfoArgs),
    /// List UTxO RPC configurations
    List,
    /// Update an existing UTxO RPC configuration
    Edit(EditArgs),
    /// Delete a UTxO RPC configuration
    Delete(DeleteArgs),
    /// Dump chain history
    DumpHistory(dump::Args),
    /// Get a specific block
    GetBlock(get_block::Args),
    /// Follow the chain's tip from a list of possible intersections
    FollowTip(follow_tip::Args),
}

#[instrument("utxorpc", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::Create(args) => create(args, ctx).await,
        Commands::Info(args) => info(args, ctx).await,
        Commands::List => list(ctx).await,
        Commands::Edit(args) => edit(args, ctx).await,
        Commands::Delete(args) => delete(args, ctx).await,
        Commands::DumpHistory(args) => dump::run(args, &ctx).await,
        Commands::GetBlock(args) => get_block::run(args, &ctx).await,
        Commands::FollowTip(args) => follow_tip::run(args, &ctx).await,
    }
}

#[derive(Parser)]
pub struct CreateArgs {
    /// Name of the UTxO RPC configuration (e.g., "preview")
    name: String,
    /// Name of the network
    network: String,
    /// URL of the UTxO RPC endpoint
    url: Url,
    /// Headers to pass to the UTxO RPC endpoint
    #[arg(short('H'), long, value_parser = crate::utils::parse_key_value, value_name = "KEY:VALUE")]
    headers: Vec<(String, String)>,
    /// If the network is a testnet (default: false)
    #[arg(short, long)]
    is_testnet: bool,
}

#[instrument(skip_all)]
async fn create(args: CreateArgs, ctx: &crate::Context) -> miette::Result<()> {
    let cfg = Utxorpc::new(
        args.name,
        args.url,
        args.network,
        args.is_testnet,
        args.headers,
    )?;

    cfg.save(&ctx.dirs, false).await?;

    info!(u5c_name = &cfg.name.raw, "UTxO RPC configured");
    println!("Created the following UTxO RPC configuration:",);
    cfg.output(&ctx.output_format);
    Ok(())
}

#[derive(Parser)]
pub struct InfoArgs {
    /// Name of the configuration
    name: String,
}

#[instrument(skip_all, fields(name=args.name))]
pub async fn info(args: InfoArgs, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.name.clone())?;
    let cfg: Option<Utxorpc> = Utxorpc::load(&ctx.dirs, &name).await?;

    match cfg {
        None => bail!(r#"Configuration named "{}" does not exist."#, &args.name,),
        Some(cfg) => cfg.output(&ctx.output_format),
    }

    Ok(())
}

#[instrument(skip_all)]
pub async fn list(ctx: &crate::Context) -> miette::Result<()> {
    let cfgs = Utxorpc::get_all_existing(&ctx.dirs).await?;
    cfgs.output(&ctx.output_format);
    Ok(())
}

#[derive(Parser)]
pub struct EditArgs {
    /// Name of the UTxO RPC configuration (e.g., "preview")
    name: String,
    /// URL of the UTxO RPC endpoint
    #[arg(short, long)]
    url: Option<Url>,
    /// Headers to pass to the UTxO RPC endpoint
    #[arg(short('H'), long, value_parser = crate::utils::parse_key_value, value_name = "KEY:VALUE")]
    headers: Option<Vec<(String, String)>>,
    /// Include this option to append the new headers to the existing headers
    #[arg(short, long, requires = "headers")]
    append: bool,
    /// If the network is a testnet (default: false)
    #[arg(short, long)]
    is_testnet: Option<bool>,
}

#[instrument(skip_all, fields(name=args.name))]
pub async fn edit(args: EditArgs, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.name.clone())?;
    let old_cfg: Option<Utxorpc> = Utxorpc::load(&ctx.dirs, &name).await?;

    match old_cfg {
        None => bail!(r#"No UTxO RPC config named "{}" exists."#, &args.name,),

        Some(mut old_cfg) => {
            if &old_cfg.name != &name {
                let should_update = inquire::Confirm::new(&format!(
                    r#"UTxO RPC config with matching or conflicting name "{}" exists, do you want to update it? Both names normalize to "{}"."#,
                    &old_cfg.name.raw,
                    &old_cfg.name.normalized()
                ))
                .with_default(false)
                .prompt()
                .into_diagnostic()?;

                if !should_update {
                    return Ok(());
                }
            }

            let headers = args.headers.map(|mut headers| {
                if args.append {
                    let mut hs = old_cfg.headers.clone();
                    hs.append(&mut headers);
                    hs
                } else {
                    headers
                }
            });

            old_cfg.update(args.url, headers, args.is_testnet);
            old_cfg.save(&ctx.dirs, true).await?;

            println!(
                r#"Updated the UTxO RPC config for "{}""#,
                &old_cfg.name().raw,
            );
            old_cfg.output(&ctx.output_format);

            Ok(())
        }
    }
}

#[derive(Parser)]
pub struct DeleteArgs {
    /// Name of the UTxO RPC configuration to delete
    name: String,
    /// Do not fail if config does not exist (default: false)
    #[arg(short, long)]
    quiet: bool,
}

#[instrument(skip_all)]
pub async fn delete(args: DeleteArgs, ctx: &crate::Context) -> miette::Result<()> {
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
