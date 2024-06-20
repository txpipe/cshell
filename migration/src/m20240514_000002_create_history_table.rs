use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TxHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TxHistory::TxHash)
                            .binary_len(32)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TxHistory::TxIndex).unsigned().not_null())
                    .col(ColumnDef::new(TxHistory::CoinDelta).binary().not_null())
                    .col(ColumnDef::new(TxHistory::Slot).binary().not_null())
                    .col(ColumnDef::new(TxHistory::BlockHash).binary().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TxHistory::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TxHistory {
    Table,
    TxHash,
    TxIndex,
    CoinDelta,
    Slot,
    BlockHash,
}
