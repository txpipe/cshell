use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use tracing::Level;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::prelude::*;

mod chain;
mod dirs;
mod transaction;
mod utils;
mod utxorpc;
mod wallet;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(
        short,
        long,
        global = true,
        help = "root dir for config and data",
        env = "CSHELL_ROOT_DIR"
    )]
    root_dir: Option<PathBuf>,

    #[arg(
        short,
        long,
        global = true,
        help = "output format for command response",
        env = "CSHELL_OUTPUT_FORMAT"
    )]
    output_format: Option<OutputFormat>,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure a UTxO RPC endpoint to use to interact with a chain
    #[command(alias = "u5c")]
    Utxorpc(utxorpc::Args),

    /// Manage Transactions
    #[command(alias = "tx")]
    Transaction(transaction::Args),

    /// Manage Wallets
    Wallet(wallet::Args),

    /// Interact with the chain through a UTxO RPC config
    Chain(chain::Args),
}

#[derive(ValueEnum, Clone)]
pub enum OutputFormat {
    Json,
    Table,
}

pub struct Context {
    pub dirs: dirs::Dirs,
    pub output_format: OutputFormat,
}
impl Context {
    fn for_cli(cli: &Cli) -> miette::Result<Self> {
        let dirs = dirs::Dirs::try_new(cli.root_dir.as_deref())?;
        let output_format = cli.output_format.clone().unwrap_or(OutputFormat::Table);

        Ok(Context {
            dirs,
            output_format,
        })
    }
}

pub fn with_tracing() {
    let indicatif_layer = IndicatifLayer::new();

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(tracing_subscriber::filter::Targets::default().with_target("cshell", Level::INFO))
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .init();
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let ctx = Context::for_cli(&cli)?;

    match cli.command {
        Commands::Utxorpc(args) => utxorpc::run(args, &ctx).await,
        Commands::Transaction(args) => transaction::run(args, &ctx).await,
        Commands::Wallet(args) => wallet::run(args, &ctx).await,
        Commands::Chain(args) => chain::run(args, &ctx).await,
    }
}
