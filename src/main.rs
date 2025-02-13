use clap::{Parser, Subcommand, ValueEnum};
use std::{borrow::Borrow, path::PathBuf};
use tracing_subscriber::{filter::LevelFilter, prelude::*};

mod output;
mod provider;
mod store;
mod transaction;
mod utils;
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
        help = "config and data path",
        env = "CSHELL_STORE_PATH"
    )]
    store_path: Option<PathBuf>,

    #[arg(
        short,
        long,
        global = true,
        help = "output format for command response",
        env = "CSHELL_OUTPUT_FORMAT"
    )]
    output_format: Option<output::OutputFormat>,

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
    /// Configure or use a provider service
    #[command(alias = "u5c")]
    Provider(provider::Args),

    /// Manage Transactions
    #[command(alias = "tx")]
    Transaction(transaction::Args),

    /// Manage Wallets
    Wallet(wallet::Args),
}

#[derive(Clone, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<&LogLevel> for LevelFilter {
    fn from(value: &LogLevel) -> Self {
        match value {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

pub struct Context {
    pub store: store::Store,
    pub output_format: output::OutputFormat,
    pub log_level: LogLevel,
}
impl Context {
    fn from_cli(cli: &Cli) -> miette::Result<Self> {
        let store = store::Store::open(cli.store_path.clone())?;
        let output_format = cli
            .output_format
            .clone()
            .unwrap_or(output::OutputFormat::Table);
        let log_level = cli.log_level.clone().unwrap_or(LogLevel::Info);

        Ok(Context {
            store,
            output_format,
            log_level,
        })
    }

    pub fn with_tracing(&self) {
        let level_filter: LevelFilter = self.log_level.borrow().into();
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::filter::Targets::default().with_target("cshell", level_filter),
            )
            .init();
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let mut ctx = Context::from_cli(&cli)?;

    match cli.command {
        Commands::Provider(args) => provider::run(args, &mut ctx).await?,
        Commands::Transaction(args) => transaction::run(args, &ctx).await?,
        Commands::Wallet(args) => wallet::run(args, &mut ctx).await?,
    };

    ctx.store.write()
}
