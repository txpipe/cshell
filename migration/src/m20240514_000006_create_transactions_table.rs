use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Transaction::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Transaction::Id)
                            .unsigned()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Transaction::TxJson).binary().not_null())
                    .col(ColumnDef::new(Transaction::TxCbor).binary())
                    .col(
                        ColumnDef::new(Transaction::Status)
                            .enumeration(Transaction::Status, STATE_VARIANTS)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Transaction::Slot).big_unsigned())
                    .col(ColumnDef::new(Transaction::Hash).string())
                    .col(ColumnDef::new(Transaction::Annotation).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Transaction::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Transaction {
    Table,
    Id,
    TxJson,
    TxCbor,
    Status,
    Slot,
    Hash,
    Annotation,
}

#[derive(DeriveIden)]
pub enum State {
    Staging,
    Built,
    Signed,
    Submitted,
    Minted,
}

const STATE_VARIANTS: [State; 5] = [
    State::Staging,
    State::Built,
    State::Signed,
    State::Submitted,
    State::Minted,
];
