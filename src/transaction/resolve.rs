use std::path::PathBuf;

use clap::Parser;
use jsonrpsee::core::params::ObjectParams;
use miette::{bail, Context, IntoDiagnostic};
use serde_json::json;
use tracing::instrument;
use tx3_lang::Protocol;

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    #[arg(long, help = "Path for TX3 file describing transaction")]
    tx3_file: PathBuf,

    #[arg(long, help = "Args for TX3 file describing transaction")]
    tx3_args_json: String,

    #[arg(long, help = "Template for TX3 file")]
    tx3_template: Option<String>,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long, help = "Path for TX3 file describing transaction")]
    provider: Option<String>,
}

#[instrument("sign", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let protocol = Protocol::from_file(args.tx3_file)
        .load()
        .into_diagnostic()
        .context("parsing tx3 file")?;

    let txs: Vec<String> = protocol.txs().map(|x| x.name.to_string()).collect();

    let template = match args.tx3_template {
        Some(template) => template,
        None => {
            let template = if txs.len() == 1 {
                txs.first().unwrap().clone()
            } else {
                inquire::Select::new("What transaction do you want to build?", txs)
                    .prompt()
                    .into_diagnostic()?
            };
            template
        }
    };

    let prototx = protocol.new_tx(&template).unwrap();

    let json_value = serde_json::from_str(&args.tx3_args_json)
        .into_diagnostic()
        .context("invalid tx3-args-json")?;

    let serde_json::Value::Object(argvalues) = json_value else {
        bail!("tx3-args-json must be an json object");
    };

    let mut builder = ObjectParams::new();
    builder
        .insert(
            "tir",
            serde_json::json!({
                "version": tx3_lang::ir::IR_VERSION.to_string(),
                "encoding": "hex",
                "bytecode": hex::encode(prototx.ir_bytes())
            }),
        )
        .unwrap();
    builder.insert("args", argvalues).unwrap();

    let response = provider.trp_resolve(&builder).await?;

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "cbor": hex::encode(&response.tx),
                }))
                .unwrap()
            );
        }
        OutputFormat::Table => println!("{}", hex::encode(&response.tx)),
    }

    Ok(())
}
