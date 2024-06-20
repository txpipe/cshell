use entity::block_history;
pub use entity::{prelude::*, protocol_parameters, recent_points, transaction, tx_history, utxo};
use futures::future::try_join_all;
pub use migration::Migrator;
use pallas::ledger::addresses::Address;
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, Database, Order, Paginator, QueryOrder, SelectModel, TransactionTrait};
use sea_orm_migration::MigratorTrait;
use std::path::{Path, PathBuf};
use tracing::error;
use types::{TransactionInfo, TxoInfo};
use utxorpc::spec::cardano::Block;
use utxorpc::spec::sync::BlockRef;

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

    pub async fn paginate_block_history(
        &self,
        order: Order,
        page_size: Option<u64>,
    ) -> Paginator<'_, DatabaseConnection, SelectModel<block_history::Model>> {
        BlockHistory::find()
            .order_by(block_history::Column::Slot, order)
            .paginate(&self.conn, page_size.unwrap_or(DEFAULT_PAGE_SIZE))
    }

    // Recent Points

    pub async fn insert_recent_points(&self, points: Vec<(u64, Vec<u8>)>) -> Result<(), DbErr> {
        let models = points
            .into_iter()
            .map(|(slot, hash)| entity::recent_points::ActiveModel {
                slot: sea_orm::ActiveValue::Set(slot as i64),
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

    // Rollback

    /// Remove all records from WalletDB created for slots after the specified
    /// slot
    pub async fn rollback_to_slot(&self, slot: u64) -> Result<(), DbErr> {
        let txn = self.conn.begin().await?;

        // UTxOs

        let point_models = RecentPoints::find()
            .filter(Condition::all().add(recent_points::Column::Slot.gte(slot)))
            .all(&txn)
            .await?;

        for point_model in point_models {
            let _ = point_model.delete(&txn).await?;
        }

        // Transaction History

        let tx_models = TxHistory::find()
            .filter(Condition::all().add(tx_history::Column::Slot.gte(slot)))
            .all(&txn)
            .await?;

        for tx_model in tx_models {
            let _ = tx_model.delete(&txn).await?;
        }

        // Recent Points

        let points_models = RecentPoints::find()
            .filter(Condition::all().add(recent_points::Column::Slot.gte(slot)))
            .all(&txn)
            .await?;

        for point_model in points_models {
            let _ = point_model.delete(&txn).await?;
        }

        // Protocol Parameters

        let pparams_models = ProtocolParameters::find()
            .filter(Condition::all().add(protocol_parameters::Column::Slot.gte(slot)))
            .all(&txn)
            .await?;

        for pparams_model in pparams_models {
            let _ = pparams_model.delete(&txn).await?;
        }

        txn.commit().await?;

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
    use miette::IntoDiagnostic;
    use pallas::ledger::addresses::Address;
    use prost::bytes::Bytes;
    use sea_orm::{Database, Order};

    use crate::wallet::dal::types::TxoInfo;

    use super::WalletDB;

    fn tx_hash() -> Vec<u8> {
        hex::decode("5d588bb46091b249f0f6874e97e3738d16e4f20f250242d6e08a93ccbf0d0e30")
            .unwrap()
            .try_into()
            .unwrap()
    }
    fn address_0() -> Vec<u8> {
        Address::from_bech32("addr1qypqwaxc9suh9g20jvv50mqkmp3gat6z890mz87g8u20eln6umxsn8n3dp3dzfc0v8jswnxddjr9unvcv6mv0cf0knjsh0j3cv")
          .into_diagnostic()
          .unwrap()
          .to_vec()
    }
    fn address_1() -> Vec<u8> {
        Address::from_bech32("addr1qxq47au29wss4g8acjk0zsmwwq0h34hhzump6stye9wuldm7nm0t6ad3jz9hy5v3smye0nvcumtzu43k7r36ag0w29qqdafvvk")
        .into_diagnostic()
        .unwrap()
        .to_vec()
    }
    fn test_utxos() -> Vec<TxoInfo> {
        vec![
            TxoInfo {
                tx_hash: Bytes::copy_from_slice(&tx_hash()),
                txo_index: 0,
                address: Bytes::copy_from_slice(&address_0()),
                slot: 49503576,
                coin: 55476850,
            },
            TxoInfo {
                tx_hash: Bytes::copy_from_slice(&tx_hash()),
                txo_index: 1,
                address: Bytes::copy_from_slice(&address_1()),
                slot: 49503576,
                coin: 1375000,
            },
        ]
    }

    #[tokio::test]
    async fn insert_utxos() {
        let sqlite_url = format!("sqlite::memory:?mode=rwc");
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

        let utxos = test_utxos();
        wallet_db.insert_utxos(&utxos).await.unwrap();

        let now_utxos = wallet_db
            .paginate_utxos(Order::Asc, None)
            .fetch()
            .await
            .unwrap();

        assert_eq!(now_utxos.len(), utxos.len());
        assert_eq!(now_utxos[0].txo_index, 0);
        assert_eq!(now_utxos[0].slot, 49503576);
        assert_eq!(now_utxos[1].txo_index, 1);
        assert_eq!(now_utxos[1].slot, 49503576);
    }

    #[tokio::test]
    async fn remove_utxos() -> miette::Result<()> {
        let sqlite_url = format!("sqlite::memory:?mode=rwc");
        let db = Database::connect(&sqlite_url).await.unwrap();

        let wallet_db = WalletDB {
            name: "test_remove_utxos".into(),
            path: sqlite_url.into(),
            conn: db,
        };

        wallet_db.migrate_up().await.unwrap();

        let utxos = test_utxos();
        wallet_db.insert_utxos(&utxos).await.into_diagnostic()?;
        let now_utxos = wallet_db
            .fetch_all_utxos(Order::Asc)
            .await
            .into_diagnostic()?;
        assert_eq!(
            utxos.len(),
            now_utxos.len(),
            "All inserted UTxOs should be fetched by the DB"
        );

        wallet_db.remove_utxos(&utxos).await.unwrap();

        let now_utxos = wallet_db
            .fetch_all_utxos(Order::Asc)
            .await
            .into_diagnostic()?;

        assert!(now_utxos.is_empty(), "All UTxOs should be removed");
        Ok(())
    }
}
