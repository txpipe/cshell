use anyhow::{bail, Result};
use clap::Parser;
use serde_json::json;
use std::path::PathBuf;
use tracing::instrument;
use tx3_sdk::{
    core::{BytesEncoding, BytesEnvelope},
    trp::{SubmitParams, TxEnvelope},
};

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for TII file describing transaction invoke interface
    #[arg(long)]
    tii_file: PathBuf,

    /// Profile to use for the transaction (as defined in the TII file)
    #[arg(long)]
    profile: Option<String>,

    /// Json string containing the invoke args for the transaction
    #[arg(long)]
    args_json: Option<String>,

    /// Path for file containing the invoke args for the transaction
    #[arg(long)]
    args_file: Option<PathBuf>,

    /// Which transaction to invoke
    #[arg(long)]
    tx_template: Option<String>,

    /// Wallets that will sign the transaction
    #[arg(long)]
    signers: Vec<String>,

    /// Skip submitting
    #[arg(long)]
    skip_submit: bool,

    /// Allow signing with unsafe wallets
    #[arg(long)]
    r#unsafe: bool,

    /// Name of the provider to use. If undefined, will use default
    #[arg(long)]
    provider: Option<String>,
}

#[instrument("invoke", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> Result<()> {
    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found")
    };

    let mut invocation = super::common::prepare_invocation(
        &args.tii_file,
        args.tx_template.as_deref(),
        args.profile.as_deref(),
    )?;

    super::common::define_args(
        &mut invocation,
        args.args_json.as_deref(),
        args.args_file.as_deref(),
        ctx,
        provider,
    )?;

    let TxEnvelope { tx, hash } = super::common::resolve_tx(invocation, provider).await?;

    let cbor = hex::decode(tx).unwrap();

    let cbor = super::common::sign_tx(&cbor, ctx, args.signers, args.r#unsafe).await?;

    if !args.skip_submit {
        provider
            .trp_submit(SubmitParams {
                tx: BytesEnvelope {
                    content: hex::encode(&cbor),
                    encoding: BytesEncoding::Hex,
                },
                witnesses: vec![],
            })
            .await?;
    }

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "hash": hash,
                    "cbor": hex::encode(&cbor),
                }))
                .unwrap()
            );
        }

        OutputFormat::Table => {
            println!("Tx Hash: {}", &hash);
            println!("Tx CBOR: {}", hex::encode(&cbor));
        }
    }

    Ok(())
}
