use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RecentPoints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RecentPoints::Id)
                            .unsigned()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RecentPoints::Slot).big_unsigned().not_null())
                    .col(
                        ColumnDef::new(RecentPoints::BlockHash)
                            .binary_len(32)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RecentPoints::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RecentPoints {
    Table,
    Id, // TODO: Remove and use slot as PK?
    Slot,
    BlockHash,
}
