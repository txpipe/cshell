use clap::{command, Parser, Subcommand};
use comfy_table::Table;
use tracing::instrument;
use utxorpc::spec::sync::{any_chain_block, AnyChainBlock};

use crate::output::OutputFormatter;

mod block;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// fetch block
    Block(block::Args),
}

#[instrument("search", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::Block(args) => block::run(args, ctx).await,
    }
}

impl OutputFormatter for Vec<AnyChainBlock> {
    fn to_table(&self) {
        for block in self {
            if let Some(chain) = &block.chain {
                match chain {
                    any_chain_block::Chain::Cardano(block) => {
                        let mut table = Table::new();
                        table.set_header(vec![
                            "Block",
                            "",
                            "Hash",
                            "Inputs",
                            "Outputs",
                            "Certificates",
                            "Ref Inputs",
                            "Datum",
                        ]);

                        if block.header.is_none() {
                            return;
                        }

                        let header = block.header.as_ref().unwrap();
                        let block_hash = hex::encode(&header.hash);
                        let block_hash_trucated = format!(
                            "{}...{}",
                            &block_hash[..4],
                            &block_hash[block_hash.len() - 4..]
                        );

                        if let Some(body) = &block.body {
                            for (i, tx) in body.tx.iter().enumerate() {
                                let hash = hex::encode(&tx.hash);
                                let inputs = tx.inputs.len();
                                let outputs = tx.outputs.len();
                                let certificates = tx.certificates.len();
                                let reference_inputs = tx.reference_inputs.len();

                                let contains_datum = if tx.outputs.iter().any(|o| {
                                    o.datum
                                        .as_ref()
                                        .map(|d| !d.hash.is_empty())
                                        .unwrap_or_default()
                                }) {
                                    "contain"
                                } else {
                                    "empty"
                                };

                                table.add_row(vec![
                                    &block_hash_trucated,
                                    &i.to_string(),
                                    &hash,
                                    &inputs.to_string(),
                                    &outputs.to_string(),
                                    &certificates.to_string(),
                                    &reference_inputs.to_string(),
                                    contains_datum,
                                ]);
                            }

                            println!("{table}");
                        }
                    }
                }
            }
        }
    }

    fn to_json(&self) {
        let result = serde_json::to_value(self);
        if let Err(err) = result {
            eprintln!("{err}");
            return;
        }

        println!(
            "{}",
            serde_json::to_string_pretty(&result.unwrap()).unwrap()
        );
    }
}
