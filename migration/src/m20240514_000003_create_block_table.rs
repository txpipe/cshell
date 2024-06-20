use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BlockHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlockHistory::Hash)
                            .binary_len(32)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BlockHistory::Slot).binary_len(8).not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BlockHistory::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BlockHistory {
    Table,
    Hash,
    Slot,
}
