use clap::ValueEnum;

#[derive(ValueEnum, Clone)]
pub enum OutputFormat {
    Json,
    Table,
}

pub trait OutputFormatter {
    fn to_table(&self);
    fn to_json(&self);

    fn output(&self, format: &OutputFormat) {
        match format {
            OutputFormat::Table => self.to_table(),
            OutputFormat::Json => self.to_json(),
        }
    }
}
