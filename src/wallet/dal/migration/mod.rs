pub use sea_orm_migration::prelude::*;

mod m20231012_000001_create_utxo_table;
mod m20231012_000002_create_history_table;
mod m20231012_000003_create_intersects_table;
mod m20231012_000004_create_pparams_table;
mod m20231102_000005_create_transactions_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20231012_000001_create_utxo_table::Migration),
            Box::new(m20231012_000002_create_history_table::Migration),
            Box::new(m20231012_000003_create_intersects_table::Migration),
            Box::new(m20231012_000004_create_pparams_table::Migration),
            Box::new(m20231102_000005_create_transactions_table::Migration),
        ]
    }
}
