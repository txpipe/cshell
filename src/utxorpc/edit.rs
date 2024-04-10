use clap::Parser;
use miette::{bail, IntoDiagnostic};
use tracing::instrument;
use url::Url;

use crate::utils::{Config, ConfigName, OutputFormatter};

use super::config::Utxorpc;

#[derive(Parser, Debug)]
pub struct Args {
    /// Name of the UTxO RPC configuration (e.g., "preview")
    name: String,
    /// URL of the UTxO RPC endpoint
    #[arg(short, long)]
    url: Option<Url>,
    /// Headers to pass to the UTxO RPC endpoint
    #[arg(short('H'), long, value_parser = crate::utils::parse_key_value, value_name = "KEY:VALUE")]
    headers: Option<Vec<(String, String)>>,
    /// Include this option to append ot the existing headers
    #[arg(short, long)]
    append: bool,
}

#[instrument("edit", skip_all, fields(name=args.name))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.name.clone())?;
    let old_cfg: Option<Utxorpc> = Utxorpc::load(&ctx.dirs, &name).await?;

    match old_cfg {
        None => bail!(r#"No UTxO RPC config named "{}" exists."#, &args.name,),

        Some(mut old_cfg) => {
            if &old_cfg.name != &name {
                let should_update = inquire::Confirm::new(&format!(
                    r#"UTxO RPC config with matching or conflicting name "{}" exists, do you want to update it? Both names normalize to "{}"."#,
                    &old_cfg.name.raw,
                    &old_cfg.name.normalized()
                ))
                .with_default(false)
                .prompt()
                .into_diagnostic()?;

                if !should_update {
                    return Ok(());
                }
            }

            let headers = args.headers.map(|mut headers| {
                if args.append {
                    let mut hs = old_cfg.headers.clone();
                    hs.append(&mut headers);
                    hs
                } else {
                    headers
                }
            });

            old_cfg.update(args.url, headers);
            old_cfg.save(&ctx.dirs, true).await?;

            println!(
                r#"Updated the UTxO RPC config for "{}""#,
                &old_cfg.name().raw,
            );
            old_cfg.output(&ctx.output_format);

            Ok(())
        }
    }
}
