use anyhow::Result;
use clap::{command, Parser, Subcommand};
use comfy_table::Table;
use tracing::instrument;
use utxorpc::spec::{cardano::Tx, query, sync};

use crate::output::OutputFormatter;

mod block;
mod transaction;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// fetch block
    Block(block::Args),

    /// fetch transaction
    Transaction(transaction::Args),
}

#[instrument("search", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> Result<()> {
    match args.command {
        Commands::Block(args) => block::run(args, ctx).await,
        Commands::Transaction(args) => transaction::run(args, ctx).await,
    }
}

fn cardano_tx_table(block_hash: Vec<u8>, tx: &[Tx]) -> Table {
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

    let block_hash = hex::encode(block_hash);
    let block_hash_trucated = format!(
        "{}...{}",
        &block_hash[..4],
        &block_hash[block_hash.len() - 4..]
    );

    for (i, tx) in tx.iter().enumerate() {
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

    table
}

impl OutputFormatter for Vec<sync::AnyChainBlock> {
    fn to_table(&self) {
        for block in self {
            if let Some(chain) = &block.chain {
                match chain {
                    sync::any_chain_block::Chain::Cardano(block) => {
                        if block.header.is_none() {
                            return;
                        }
                        let header = block.header.as_ref().unwrap();
                        if let Some(body) = &block.body {
                            let table = cardano_tx_table(header.hash.clone().into(), &body.tx);
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

impl OutputFormatter for Vec<query::AnyChainBlock> {
    fn to_table(&self) {
        for block in self {
            if let Some(chain) = &block.chain {
                match chain {
                    query::any_chain_block::Chain::Cardano(block) => {
                        if block.header.is_none() {
                            return;
                        }
                        let header = block.header.as_ref().unwrap();
                        if let Some(body) = &block.body {
                            let table = cardano_tx_table(header.hash.clone().into(), &body.tx);
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

impl OutputFormatter for query::AnyChainTx {
    fn to_table(&self) {
        if let Some(chain) = &self.chain {
            match chain {
                query::any_chain_tx::Chain::Cardano(tx) => {
                    let block_hash = self.block_ref.as_ref().unwrap().hash.clone();
                    let table = cardano_tx_table(block_hash.into(), &[tx.clone()]);
                    println!("{table}");
                }
            }
        }
    }

    fn to_json(&self) {
        if let Some(chain) = &self.chain {
            match chain {
                query::any_chain_tx::Chain::Cardano(tx) => {
                    let result = serde_json::to_value(tx);
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
        }
    }
}
