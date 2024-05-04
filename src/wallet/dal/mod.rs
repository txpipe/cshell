pub use entity::{prelude::*, protocol_parameters, recent_points, transaction, tx_history, utxo};
use futures::future::{join_all, try_join_all};
pub use migration::Migrator;
use tracing::error;
use utxorpc::spec::cardano::Block;
use utxorpc::spec::sync::BlockRef;

use std::num;
use std::path::{Path, PathBuf};

use pallas::ledger::addresses::Address;
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, Database, Order, Paginator, QueryOrder, SelectModel, TransactionTrait};
use sea_orm_migration::MigratorTrait;
use types::{TransactionInfo, TxoInfo};

pub mod types;

const DEFAULT_PAGE_SIZE: u64 = 20;
const DEFAULT_POINTS_SPREAD_SIZE: u32 = 20;

pub struct WalletDB {
    pub name: String,
    pub path: PathBuf,
    pub conn: DatabaseConnection,
}

impl WalletDB {
    pub async fn open(name: &str, path: &Path) -> Result<Self, DbErr> {
        let sqlite_url = format!("sqlite:{}/state.sqlite?mode=rwc", path.display());
        let db = Database::connect(sqlite_url).await?;

        let out = Self {
            name: name.to_owned(),
            path: path.to_path_buf(),
            conn: db,
        };

        out.migrate_up().await?;

        Ok(out)
    }

    pub async fn migrate_up(&self) -> Result<(), DbErr> {
        Migrator::up(&self.conn, None).await
    }

    // UTxOs

    pub async fn insert_utxos(&self, utxos: &Vec<TxoInfo>) -> Result<(), DbErr> {
        if utxos.is_empty() {
            return Ok(());
        }

        let models: Vec<utxo::ActiveModel> = utxos
            .into_iter()
            .map(|info| info.as_active_model())
            .collect();

        Utxo::insert_many(models).exec(&self.conn).await?;
        Ok(())
    }

    pub async fn remove_utxos(&self, utxos: &Vec<TxoInfo>) -> Result<Vec<utxo::Model>, DbErr> {
        // Early exit to prevent all UTxOs being returned by blanket `any` condition
        if utxos.is_empty() {
            return Ok(vec![]);
        }

        let txn = self.conn.begin().await?;

        let condition = utxos.iter().fold(Condition::any(), |condition, utxo| {
            condition
                .add(utxo::Column::TxHash.eq(utxo.tx_hash.to_vec()))
                .add(utxo::Column::TxoIndex.eq(utxo.txo_index))
        });

        let found_utxos = Utxo::find().filter(condition.clone()).all(&txn).await?;

        let deleted_count = Utxo::delete_many()
            .filter(condition)
            .exec(&txn)
            .await?
            .rows_affected;

        if deleted_count != found_utxos.len() as u64 {
            error!(
                "The wrong number of UTxOs were deleted.
                {deleted_count} UTxOs were deleted, but these {} UTxOs were found:{:?}",
                found_utxos.len(),
                found_utxos
            );
        }

        txn.commit().await?;

        Ok(found_utxos)
    }

    pub async fn resolve_utxo(
        &self,
        tx_hash: &[u8],
        txo_index: u32,
    ) -> Result<Option<TxoInfo>, DbErr> {
        Utxo::find()
            .filter(
                Condition::all()
                    .add(utxo::Column::TxHash.eq(tx_hash))
                    .add(utxo::Column::TxoIndex.eq(txo_index)),
            )
            .one(&self.conn)
            .await
            .map(|res| res.map(TxoInfo::from))
    }

    pub fn paginate_utxos(
        &self,
        order: Order,
        page_size: Option<u64>,
    ) -> Paginator<'_, DatabaseConnection, SelectModel<utxo::Model>> {
        Utxo::find()
            .order_by(utxo::Column::Slot, order)
            .paginate(&self.conn, page_size.unwrap_or(DEFAULT_PAGE_SIZE))
    }

    #[allow(unused)]
    pub async fn paginate_utxos_for_address(
        &self,
        address: Address,
        order: Order,
        page_size: Option<u64>,
    ) -> Paginator<'_, DatabaseConnection, SelectModel<utxo::Model>> {
        Utxo::find()
            .filter(utxo::Column::Address.eq(address.to_vec()))
            .order_by(utxo::Column::Slot, order.clone())
            .paginate(&self.conn, page_size.unwrap_or(DEFAULT_PAGE_SIZE))
    }

    pub async fn fetch_all_utxos(&self, order: Order) -> Result<Vec<TxoInfo>, DbErr> {
        let models = Utxo::find()
            .order_by(utxo::Column::Slot, order)
            .all(&self.conn)
            .await?;

        Ok(models.into_iter().map(|model| model.into()).collect())
    }

    // Transaction History

    // TODO: balance type
    pub async fn insert_history_txs(&self, txs: &Vec<TransactionInfo>) -> Result<(), DbErr> {
        if txs.is_empty() {
            Ok(())
        } else {
            let models = txs.iter().map(|info| info.as_active_model());
            TxHistory::insert_many(models)
                .exec(&self.conn)
                .await
                .map(|_| {})
        }
    }

    pub fn paginate_tx_history(
        &self,
        order: Order,
        page_size: Option<u64>,
    ) -> Paginator<'_, DatabaseConnection, SelectModel<tx_history::Model>> {
        TxHistory::find()
            .order_by(tx_history::Column::Slot, order.clone())
            .order_by(tx_history::Column::TxIndex, order)
            .paginate(&self.conn, page_size.unwrap_or(DEFAULT_PAGE_SIZE))
    }

    // Blocks

    pub async fn insert_blocks(&self, blocks: &Vec<Block>) -> Result<(), DbErr> {
        if blocks.is_empty() {
            Ok(())
        } else {
            let models = blocks.iter().map(types::block_to_model);

            BlockHistory::insert_many(models)
                .exec(&self.conn)
                .await
                .map(|_| {})
        }
    }

    // Recent Points

    pub async fn insert_recent_points(&self, points: Vec<(u64, Vec<u8>)>) -> Result<(), DbErr> {
        let models = points
            .into_iter()
            .map(|(slot, hash)| entity::recent_points::ActiveModel {
                slot: sea_orm::ActiveValue::Set(slot as i64), // TODO Why i64?
                block_hash: sea_orm::ActiveValue::Set(hash),
            });

        RecentPoints::insert_many(models).exec(&self.conn).await?;
        Ok(())
    }

    pub async fn get_most_recent_point(&self) -> Result<Option<BlockRef>, DbErr> {
        let model = RecentPoints::find()
            .order_by_desc(recent_points::Column::Slot)
            .one(&self.conn)
            .await?;

        Ok(model.as_ref().map(types::block_ref_from_recent_point))
    }

    pub async fn get_recent_points_spread(
        &self,
        num_points: Option<u32>,
    ) -> Result<Vec<BlockRef>, DbErr> {
        let page_size = DEFAULT_PAGE_SIZE;

        let paginated_points = RecentPoints::find()
            .order_by_desc(recent_points::Column::Slot)
            .paginate(&self.conn, page_size);
        let paginated_points = std::sync::Arc::new(paginated_points);

        let indices =
            (0..num_points.unwrap_or(DEFAULT_POINTS_SPREAD_SIZE)).map(|n| (2 as u64).pow(n));

        let points_spread: Vec<_> = try_join_all(indices.map(move |i| {
            let paginated_points = std::sync::Arc::clone(&paginated_points);
            async move {
                let points = paginated_points.fetch_page(i / page_size).await?;
                let point = points.get(i as usize % page_size as usize);
                Ok::<Option<recent_points::Model>, DbErr>(point.cloned())
            }
        }))
        .await?;

        Ok(points_spread
            .iter()
            .flatten()
            .map(types::block_ref_from_recent_point)
            .collect())
    }

    pub async fn remove_recent_points_before_slot(&self, slot: u64) -> Result<(), DbErr> {
        let txn = self.conn.begin().await?;

        let point_models = RecentPoints::find()
            .filter(Condition::all().add(recent_points::Column::Slot.lt(slot)))
            .all(&txn)
            .await?;

        for point_model in point_models {
            let _ = point_model.delete(&txn).await?;
        }

        txn.commit().await
    }

    // Protocol Parameters

    pub async fn insert_protocol_parameters(
        &self,
        slot: u64,
        tx_block_index: u16,
        update_cbor: Vec<u8>,
    ) -> Result<(), DbErr> {
        let pparams_model = entity::protocol_parameters::ActiveModel {
            slot: sea_orm::ActiveValue::Set(slot as i64),
            block_index: sea_orm::ActiveValue::Set(tx_block_index.into()),
            update_cbor: sea_orm::ActiveValue::Set(update_cbor),
            ..Default::default()
        };

        let _ = ProtocolParameters::insert(pparams_model)
            .exec(&self.conn)
            .await?;

        Ok(())
    }

    /// Fetch the CBOR of the most recent protocol parameters seen on-chain
    pub async fn fetch_latest_protocol_parameters(&self) -> Result<Option<Vec<u8>>, DbErr> {
        let res = ProtocolParameters::find()
            .order_by_desc(protocol_parameters::Column::Slot)
            .order_by_desc(protocol_parameters::Column::BlockIndex)
            .one(&self.conn)
            .await?;

        Ok(res.map(|r| r.update_cbor))
    }

    // Rollback

    /// Remove all records from WalletDB created for slots after the specified
    /// slot
    pub async fn rollback_to_slot(&self, slot: u64) -> Result<(), DbErr> {
        let txn = self.conn.begin().await?;

        // UTxOs

        let point_models = RecentPoints::find()
            .filter(Condition::all().add(recent_points::Column::Slot.gt(slot)))
            .all(&txn)
            .await?;

        for point_model in point_models {
            let _ = point_model.delete(&txn).await?;
        }

        // Transaction History

        let tx_models = TxHistory::find()
            .filter(Condition::all().add(tx_history::Column::Slot.gt(slot)))
            .all(&txn)
            .await?;

        for tx_model in tx_models {
            let _ = tx_model.delete(&txn).await?;
        }

        // Recent Points

        let points_models = RecentPoints::find()
            .filter(Condition::all().add(recent_points::Column::Slot.gt(slot)))
            .all(&txn)
            .await?;

        for point_model in points_models {
            let _ = point_model.delete(&txn).await?;
        }

        // Protocol Parameters

        let pparams_models = ProtocolParameters::find()
            .filter(Condition::all().add(protocol_parameters::Column::Slot.gt(slot)))
            .all(&txn)
            .await?;

        for pparams_model in pparams_models {
            let _ = pparams_model.delete(&txn).await?;
        }

        Ok(())
    }

    // Transactions

    pub async fn insert_transaction(&self, tx_json: Vec<u8>) -> Result<i32, DbErr> {
        let transaction_model = entity::transaction::ActiveModel {
            tx_json: sea_orm::ActiveValue::Set(tx_json),
            status: sea_orm::ActiveValue::Set(transaction::Status::Staging),
            ..Default::default()
        };

        let result = Transaction::insert(transaction_model)
            .exec(&self.conn)
            .await?;

        Ok(result.last_insert_id)
    }

    pub fn paginate_transactions(
        &self,
        order: Order,
        page_size: Option<u64>,
    ) -> Paginator<'_, DatabaseConnection, SelectModel<transaction::Model>> {
        Transaction::find()
            .order_by(transaction::Column::Id, order.clone())
            .paginate(&self.conn, page_size.unwrap_or(DEFAULT_PAGE_SIZE))
    }

    pub async fn fetch_by_id(&self, id: &i32) -> Result<Option<transaction::Model>, DbErr> {
        Transaction::find_by_id(*id).one(&self.conn).await
    }

    pub async fn remove_transaction(&self, id: &i32) -> Result<(), DbErr> {
        Transaction::delete_by_id(*id).exec(&self.conn).await?;
        Ok(())
    }

    pub async fn update_transaction(&self, model: transaction::Model) -> Result<(), DbErr> {
        let model: entity::transaction::ActiveModel = model.into();

        Transaction::update(model.reset_all())
            .exec(&self.conn)
            .await?;

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use pallas::ledger::{
        primitives::babbage::TransactionInput,
        traverse::{Era, MultiEraInput, MultiEraOutput},
    };
    use sea_orm::{Database, Order};

    use super::WalletDB;

    #[tokio::test]
    async fn insert_utxos() {
        let sqlite_url = format!("sqlite:/tmp/test_insert_utxos.sqlite?mode=rwc");
        let db = Database::connect(&sqlite_url).await.unwrap();

        let wallet_db = WalletDB {
            name: "test_utxos".into(),
            path: sqlite_url.into(),
            conn: db,
        };

        wallet_db.migrate_up().await.unwrap();

        let init_utxos = wallet_db
            .paginate_utxos(Order::Asc, None)
            .fetch()
            .await
            .unwrap();

        assert!(init_utxos.is_empty());

        let hash_0: [u8; 32] =
            hex::decode("5d588bb46091b249f0f6874e97e3738d16e4f20f250242d6e08a93ccbf0d0e30")
                .unwrap()
                .try_into()
                .unwrap();
        let index_0 = 2;
        let utxo_cbor_0 = hex::decode("82583901576aefddef29b4168f74b78879404b62e98ce7b761874130fb48b996096c02a359fc0ab647b202a0351269ea72e84061b2ad3b40f00067c4821a00169b08a1581cec2e1c314ee754cea4ba3afc69f74b2130f87bb3928e1a1e8534c209a14f526167696e675465656e303331313901").unwrap();
        let utxo_0 = MultiEraOutput::decode(Era::Alonzo, &utxo_cbor_0).unwrap();
        let slot_0 = 49503576;

        let hash_1: [u8; 32] =
            hex::decode("5d588bb46091b249f0f6874e97e3738d16e4f20f250242d6e08a93ccbf0d0e30")
                .unwrap()
                .try_into()
                .unwrap();
        let index_1 = 3;
        let utxo_cbor_1 = hex::decode("82583901576aefddef29b4168f74b78879404b62e98ce7b761874130fb48b996096c02a359fc0ab647b202a0351269ea72e84061b2ad3b40f00067c4821a0c507ff2a4581cb000e9f3994de3226577b4d61280994e53c07948c8839d628f4a425aa14f436c756d737947686f73747335343501581cc364930bd612f42e14d156e1c5410511e77f64cab8f2367a9df544d1a154426f7373436174526f636b6574436c756238393001581cec2e1c314ee754cea4ba3afc69f74b2130f87bb3928e1a1e8534c209af4e526167696e675465656e30303132014f526167696e675465656e3030383838014f526167696e675465656e3031303834014f526167696e675465656e3031333836014f526167696e675465656e3031363330014f526167696e675465656e3031363434014f526167696e675465656e3031393435014f526167696e675465656e3031393933014f526167696e675465656e3032333535014f526167696e675465656e3032363533014f526167696e675465656e3033303233014f526167696e675465656e3033353633014f526167696e675465656e3033383039014f526167696e675465656e3034313731014f526167696e675465656e303437393201581cf0ff48bbb7bbe9d59a40f1ce90e9e9d0ff5002ec48f232b49ca0fb9aa14b63617264616e6f2e61646101").unwrap();
        let utxo_1 = MultiEraOutput::decode(Era::Alonzo, &utxo_cbor_1).unwrap();
        let slot_1 = 49503576;

        let utxos = vec![
            (hash_0, index_0, utxo_0, slot_0, Era::Alonzo),
            (hash_1, index_1, utxo_1, slot_1, Era::Alonzo),
        ];

        // wallet_db.insert_utxos(utxos).await.unwrap(); // TODO

        let now_utxos = wallet_db
            .paginate_utxos(Order::Asc, None)
            .fetch()
            .await
            .unwrap();

        assert_eq!(now_utxos.len(), 2);
        assert_eq!(now_utxos[0].txo_index, 2);
        assert_eq!(now_utxos[0].slot, 49503576);
        assert_eq!(now_utxos[1].txo_index, 3);
        assert_eq!(now_utxos[1].slot, 49503576);

        drop(wallet_db);

        std::fs::remove_file("/tmp/test_insert_utxos.sqlite").unwrap();
    }

    #[tokio::test]
    async fn remove_utxos() {
        let sqlite_url = format!("sqlite:/tmp/test_remove_utxos.sqlite?mode=rwc");
        let db = Database::connect(&sqlite_url).await.unwrap();

        let wallet_db = WalletDB {
            name: "test_remove_utxos".into(),
            path: sqlite_url.into(),
            conn: db,
        };

        wallet_db.migrate_up().await.unwrap();

        let init_utxos = wallet_db
            .paginate_utxos(Order::Asc, None)
            .fetch()
            .await
            .unwrap();

        let mut to_remove = vec![];

        for utxo in init_utxos {
            let tx_hash: [u8; 32] = utxo.tx_hash.try_into().unwrap();

            let txin = TransactionInput {
                transaction_id: tx_hash.into(),
                index: utxo.txo_index.try_into().unwrap(),
            };

            to_remove.push(txin);
        }

        // wallet_db
        //     .remove_utxos(
        //         to_remove
        //             .iter()
        //             .map(MultiEraInput::from_alonzo_compatible)
        //             .collect(),
        //     )
        //     .await
        //     .unwrap(); // TODO

        let now_utxos = wallet_db
            .paginate_utxos(Order::Asc, None)
            .fetch()
            .await
            .unwrap();

        assert!(now_utxos.is_empty());

        drop(wallet_db);

        std::fs::remove_file("/tmp/test_remove_utxos.sqlite").unwrap();
    }
}
