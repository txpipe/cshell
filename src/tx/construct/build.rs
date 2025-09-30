use std::{fs, path::PathBuf};

use anyhow::{Result, Context};
use clap::Parser;
use tracing::instrument;

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for tx3 file to create the transaction
    #[arg(long)]
    tx3_file: PathBuf,
}

#[instrument("build", skip_all)]
pub async fn run(args: Args, _ctx: &crate::Context) -> Result<()> {
    let ast_path_buf = args.tx3_file.with_extension("ast");

    let tx_builder = super::common::TransactionBuilder::from_ast(&ast_path_buf)?;

    // Generate the tx3 content
    let tx3_content = tx_builder.generate_tx3_content();

    // Write to file
    fs::write(&args.tx3_file, tx3_content)
        .context("Failed to write tx3 file")?;

    println!("\n✅ Transaction created successfully!");
    println!("📄 File saved to: {}", args.tx3_file.display());

    Ok(())
}