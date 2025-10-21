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

#[instrument("add-output", skip_all)]
pub async fn run(args: Args, _ctx: &crate::Context) -> Result<()> {
    let ast_path_buf = args.tx3_file.with_extension("ast");

    let mut tx_builder = super::common::TransactionBuilder::from_ast(&ast_path_buf)?;

    tx_builder.collect_outputs(true)?;

    // Serialize and write AST
    let ast_json = serde_json::to_string_pretty(&tx_builder.ast)
        .context("Failed to serialize tx3 AST")?;

    fs::write(&ast_path_buf, ast_json)
        .with_context(|| format!("Failed to write tx3 AST file: {}", ast_path_buf.display()))?;

    println!("\n✅ Output added successfully!");
    println!("📄 File saved to: {}", ast_path_buf.display());

    Ok(())
}