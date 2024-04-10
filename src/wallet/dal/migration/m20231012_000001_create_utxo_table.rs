use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Utxo::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Utxo::Id)
                            .unsigned()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Utxo::TxHash).binary_len(32).not_null())
                    .col(ColumnDef::new(Utxo::TxoIndex).unsigned().not_null())
                    .col(ColumnDef::new(Utxo::PaymentCred).binary_len(28).not_null())
                    .col(ColumnDef::new(Utxo::FullAddress).binary().not_null())
                    .col(ColumnDef::new(Utxo::Slot).big_unsigned().not_null())
                    .col(ColumnDef::new(Utxo::Era).small_unsigned().not_null())
                    .col(ColumnDef::new(Utxo::Cbor).binary().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Utxo::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Utxo {
    Table,
    Id,
    TxHash,
    TxoIndex,
    PaymentCred,
    FullAddress, // TODO: Reflect in ADR, easier than having field which can support all different staking cred
    Slot,
    Era,
    Cbor,
}
