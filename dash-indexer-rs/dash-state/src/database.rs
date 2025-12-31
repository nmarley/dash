use crate::{BlockInfo, Result, StateError, TransactionInfo};
use bitcoin::{BlockHash, Txid};
use lmdb::{Database, DatabaseFlags, Environment, EnvironmentFlags, Transaction, WriteFlags};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

const MAX_DBS: u32 = 10;
const DB_SIZE: usize = 100 * 1024 * 1024 * 1024; // 100 GB

/// LMDB database for indexing Dash blockchain data
pub struct IndexDatabase {
    env: Arc<Environment>,
    tx_index: Database,
    block_index: Database,
    height_index: Database,
}

impl IndexDatabase {
    /// Open or create the database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        info!("Opening index database at {:?}", path.as_ref());

        std::fs::create_dir_all(&path)?;

        let env = Environment::new()
            .set_flags(EnvironmentFlags::NO_TLS | EnvironmentFlags::WRITE_MAP)
            .set_max_dbs(MAX_DBS)
            .set_map_size(DB_SIZE)
            .open(path.as_ref())?;

        let tx_index = env.create_db(Some("tx_index"), DatabaseFlags::empty())?;
        let block_index = env.create_db(Some("block_index"), DatabaseFlags::empty())?;
        let height_index = env.create_db(Some("height_index"), DatabaseFlags::INTEGER_KEY)?;

        Ok(Self {
            env: Arc::new(env),
            tx_index,
            block_index,
            height_index,
        })
    }

    /// Store a transaction in the index
    pub fn put_transaction(&self, tx_info: &TransactionInfo) -> Result<()> {
        let txn = self.env.begin_rw_txn()?;

        let key = tx_info.txid.as_byte_array();
        let value = tx_info.to_bytes();

        txn.put(self.tx_index, &key, &value, WriteFlags::empty())?;
        txn.commit()?;

        debug!("Stored transaction: {}", tx_info.txid);
        Ok(())
    }

    /// Get a transaction from the index
    pub fn get_transaction(&self, txid: &Txid) -> Result<TransactionInfo> {
        let txn = self.env.begin_ro_txn()?;
        let key = txid.as_byte_array();

        let value = txn
            .get(self.tx_index, &key)
            .map_err(|_| StateError::TxNotFound(txid.to_string()))?;

        TransactionInfo::from_bytes(value)
    }

    /// Store a block in the index
    pub fn put_block(&self, block_info: &BlockInfo) -> Result<()> {
        let txn = self.env.begin_rw_txn()?;

        // Store by block hash
        let hash_key = block_info.hash.as_byte_array();
        let value = block_info.to_bytes();
        txn.put(self.block_index, &hash_key, &value, WriteFlags::empty())?;

        // Store height -> hash mapping
        let height_key = block_info.height.to_be_bytes();
        let height_value = block_info.hash.as_byte_array();
        txn.put(
            self.height_index,
            &height_key,
            height_value,
            WriteFlags::empty(),
        )?;

        txn.commit()?;

        debug!(
            "Stored block: {} at height {}",
            block_info.hash, block_info.height
        );
        Ok(())
    }

    /// Get a block by hash
    pub fn get_block_by_hash(&self, hash: &BlockHash) -> Result<BlockInfo> {
        let txn = self.env.begin_ro_txn()?;
        let key = hash.as_byte_array();

        let value = txn
            .get(self.block_index, &key)
            .map_err(|_| StateError::BlockNotFound(hash.to_string()))?;

        BlockInfo::from_bytes(value)
    }

    /// Get a block by height
    pub fn get_block_by_height(&self, height: u32) -> Result<BlockInfo> {
        let txn = self.env.begin_ro_txn()?;
        let height_key = height.to_be_bytes();

        let hash_bytes = txn
            .get(self.height_index, &height_key)
            .map_err(|_| StateError::BlockNotFound(format!("height {}", height)))?;

        let hash = BlockHash::from_slice(hash_bytes)
            .map_err(|e| StateError::InvalidData(format!("Invalid block hash: {}", e)))?;

        self.get_block_by_hash(&hash)
    }

    /// Get the current tip height
    pub fn get_tip_height(&self) -> Result<Option<u32>> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.height_index)?;

        // Get the last entry (highest height)
        match cursor.get(None, None, lmdb::cursor::MoveOp::Last) {
            Ok((key_bytes, _)) => {
                if key_bytes.len() == 4 {
                    let height = u32::from_be_bytes([
                        key_bytes[0],
                        key_bytes[1],
                        key_bytes[2],
                        key_bytes[3],
                    ]);
                    Ok(Some(height))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Sync the database to disk
    pub fn sync(&self) -> Result<()> {
        self.env.sync(true)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;
    use tempfile::TempDir;

    #[test]
    fn test_database_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = IndexDatabase::new(temp_dir.path()).unwrap();

        // Create test data
        let txid = Txid::all_zeros();
        let block_hash = BlockHash::all_zeros();
        let tx_info =
            TransactionInfo::new(txid, block_hash, 100, 0, crate::DashTxType::Normal, None);

        // Store and retrieve transaction
        db.put_transaction(&tx_info).unwrap();
        let retrieved = db.get_transaction(&txid).unwrap();
        assert_eq!(retrieved.height, 100);

        // Store and retrieve block
        let block_info = BlockInfo::new(block_hash, 100, vec![0u8; 80], 1);
        db.put_block(&block_info).unwrap();

        let retrieved_block = db.get_block_by_hash(&block_hash).unwrap();
        assert_eq!(retrieved_block.height, 100);

        let retrieved_by_height = db.get_block_by_height(100).unwrap();
        assert_eq!(retrieved_by_height.hash, block_hash);

        // Check tip height
        let tip = db.get_tip_height().unwrap();
        assert_eq!(tip, Some(100));
    }
}
