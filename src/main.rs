use clap::{Parser, Subcommand, ValueEnum};
use std::{borrow::Borrow, path::PathBuf};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{filter::LevelFilter, prelude::*};
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

    #[arg(
        long,
        help = "Control the verbosity of CShell logging",
        env = "CSHELL_LOG",
        global = true
    )]
    log_level: Option<LogLevel>,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure or use a UTxO RPC service
    #[command(alias = "u5c")]
    Utxorpc(utxorpc::Args),

    /// Manage Transactions
    #[command(alias = "tx")]
    Transaction(transaction::Args),

    /// Manage Wallets
    Wallet(wallet::Args),
}

#[derive(ValueEnum, Clone)]
pub enum OutputFormat {
    Json,
    Table,
}

#[derive(Clone, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Into<LevelFilter> for &LogLevel {
    fn into(self) -> tracing_subscriber::filter::LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

pub struct Context {
    pub dirs: dirs::Dirs,
    pub output_format: OutputFormat,
    log_level: LogLevel,
}
impl Context {
    fn for_cli(cli: &Cli) -> miette::Result<Self> {
        let dirs = dirs::Dirs::try_new(cli.root_dir.as_deref())?;
        let output_format = cli.output_format.clone().unwrap_or(OutputFormat::Table);
        let log_level = cli
            .log_level
            .as_ref()
            .map(|ll| ll.clone())
            .unwrap_or(LogLevel::Info);

        Ok(Context {
            dirs,
            output_format,
            log_level,
        })
    }

    pub fn with_tracing(&self) {
        let indicatif_layer = IndicatifLayer::new();
        let level_filter: LevelFilter = self.log_level.borrow().into();

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::filter::Targets::default().with_target("cshell", level_filter),
            )
            .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
            .with(indicatif_layer)
            .init();
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let ctx = Context::for_cli(&cli)?;

    match cli.command {
        Commands::Utxorpc(args) => utxorpc::run(args, &ctx).await,
        Commands::Transaction(args) => transaction::run(args, &ctx).await,
        Commands::Wallet(args) => wallet::run(args, &ctx).await,
    }
}
