use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProtocolParameters::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProtocolParameters::Id)
                            .unsigned()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProtocolParameters::Slot)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProtocolParameters::BlockIndex)
                            .unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProtocolParameters::UpdateCbor)
                            .binary()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProtocolParameters::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProtocolParameters {
    Table,
    Id,
    Slot,
    BlockIndex,
    UpdateCbor,
}
