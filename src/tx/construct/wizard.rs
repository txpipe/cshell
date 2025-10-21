use std::{fs, path::PathBuf};

use anyhow::{Result, Context};
use clap::Parser;
use tracing::instrument;
use inquire::Confirm;

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for tx3 file to create the transaction
    #[arg(long)]
    tx3_file: PathBuf,
}

#[instrument("wizard", skip_all)]
pub async fn run(args: Args, _ctx: &crate::Context) -> Result<()> {
    let ast_path_buf = args.tx3_file.with_extension("ast");

    if args.tx3_file.exists() {
        println!("⚠️  Warning: The specified tx3 file already exists and will be overwritten.");
        let proceed = Confirm::new("Do you want to continue?")
            .with_default(false)
            .prompt()?;

        if !proceed {
            println!("Operation cancelled by user.");
            return Ok(());
        }
    }

    let mut tx_builder = super::common::TransactionBuilder::from_ast(&ast_path_buf)?;

    tx_builder.collect_inputs(false)?;

    tx_builder.collect_outputs(false)?;

    let ast = tx_builder.ast.clone();

    // Generate the tx3 content
    let tx3_content = tx_builder.generate_tx3_content();

    // Write to file
    fs::write(&args.tx3_file, tx3_content)
        .context("Failed to write tx3 file")?;

    // Serialize and write AST
    let ast_json = serde_json::to_string_pretty(&ast)
        .context("Failed to serialize tx3 AST")?;

    fs::write(&ast_path_buf, ast_json)
        .with_context(|| format!("Failed to write tx3 AST file: {}", ast_path_buf.display()))?;

    println!("\n✅ Transaction created successfully!");
    println!("📄 File saved to: {}", args.tx3_file.display());

    Ok(())
}
