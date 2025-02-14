use clap::Parser;

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to show the balance of. If undefined, will use default
    name: String,
}

pub async fn run(_args: Args, _ctx: &crate::Context) -> miette::Result<()> {
    todo!()
}
