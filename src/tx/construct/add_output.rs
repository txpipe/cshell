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

    let ast = tx_builder.ast.clone();

    // Write to AST file
    fs::write(&ast_path_buf, serde_json::to_string_pretty(&ast).unwrap())
        .context("Failed to write tx3 AST file")?;

    println!("\n✅ Output added successfully!");
    println!("📄 File saved to: {}", ast_path_buf.display());

    Ok(())
}