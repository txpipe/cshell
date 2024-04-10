use std::fmt::Display;

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum Status {
    #[sea_orm(string_value = "staging")]
    Staging,
    #[sea_orm(string_value = "built")]
    Built,
    #[sea_orm(string_value = "signed")]
    Signed,
    #[sea_orm(string_value = "submitted")]
    Submitted,
    #[sea_orm(string_value = "minted")]
    Minted,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "transaction")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))")]
    pub tx_json: Vec<u8>,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))", nullable)]
    pub tx_cbor: Option<Vec<u8>>,
    pub status: Status,
    pub slot: Option<i64>,
    pub hash: Option<String>,
    pub annotation: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Staging => write!(f, "Staging"),
            Status::Built => write!(f, "Built"),
            Status::Signed => write!(f, "Signed"),
            Status::Submitted => write!(f, "Submitted"),
            Status::Minted => write!(f, "Minted"),
        }
    }
}
