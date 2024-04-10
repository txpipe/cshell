//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.4

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "protocol_parameters")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub slot: i64,
    pub block_index: i32,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))")]
    pub update_cbor: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
