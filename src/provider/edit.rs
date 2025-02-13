use clap::Parser;

#[derive(Parser)]
pub struct Args {
    /// Name of the provider to edit
    name: String,
}

pub async fn run(_args: Args, _ctx: &crate::Context) -> miette::Result<()> {
    todo!()
}
