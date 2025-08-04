use crate::output::OutputFormatter;

pub async fn run(ctx: &crate::Context) -> anyhow::Result<()> {
    ctx.store.providers().output(&ctx.output_format);
    Ok(())
}
