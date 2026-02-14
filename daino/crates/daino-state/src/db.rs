//! LMDB-backed database for Daino blockchain indexer.
//!
//! Stores and retrieves:
//! - Blocks by height and hash
//! - Transactions by txid
//! - Chain tip metadata
//!
//! Uses heed (safe LMDB wrapper) for crash-safe, memory-mapped storage.

use std::path::Path;

use anyhow::{Context, Result};
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use librustdash::hash::hash_to_display;

/// A stored block record. We store the essential header fields plus
/// the raw serialized block for re-parsing when needed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockRecord {
    /// Block height
    pub height: u32,
    /// Block hash (internal byte order -- X11 hash from dashd, not computed)
    pub hash: [u8; 32],
    /// Previous block hash (internal byte order)
    pub prev_hash: [u8; 32],
    /// Merkle root (internal byte order)
    pub merkle_root: [u8; 32],
    /// Block timestamp
    pub time: u32,
    /// Difficulty target (compact)
    pub bits: u32,
    /// Nonce
    pub nonce: u32,
    /// Number of transactions in the block
    pub tx_count: u32,
    /// Total block size in bytes
    pub size: u32,
}

/// A stored transaction record. Stores location info for the tx
/// so we can look it up by txid without storing the full raw tx.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TxRecord {
    /// Transaction ID (internal byte order)
    pub txid: [u8; 32],
    /// Block height containing this transaction
    pub block_height: u32,
    /// Index within the block's transaction list
    pub tx_index: u32,
    /// Transaction version
    pub version: i16,
    /// Transaction type (Dash tx type as u16)
    pub tx_type: u16,
    /// Lock time
    pub lock_time: u32,
    /// Total output value in satoshis
    pub value_out: i64,
    /// Number of inputs
    pub input_count: u32,
    /// Number of outputs
    pub output_count: u32,
}

/// Chain metadata stored in the meta database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChainMeta {
    /// Current chain tip height
    pub tip_height: u32,
    /// Current chain tip hash (internal byte order)
    pub tip_hash: [u8; 32],
    /// Total number of indexed blocks
    pub block_count: u32,
    /// Total number of indexed transactions
    pub tx_count: u64,
}

/// The Daino database -- LMDB-backed storage for blockchain data.
pub struct DainoDB {
    env: Env,
    /// height (4 bytes BE) -> BlockRecord (bincode)
    blocks_by_height: Database<Bytes, Bytes>,
    /// block_hash (32 bytes) -> height (4 bytes BE)
    hash_to_height: Database<Bytes, Bytes>,
    /// txid (32 bytes) -> TxRecord (bincode)
    txs_by_id: Database<Bytes, Bytes>,
    /// "meta" -> ChainMeta (bincode)
    meta: Database<Str, Bytes>,
}

/// Maximum database size: 10 GB. LMDB pre-allocates virtual address space
/// (not physical memory), so this is safe to set large.
const MAX_DB_SIZE: usize = 10 * 1024 * 1024 * 1024;

/// Number of named databases we use.
const MAX_DBS: u32 = 10;

impl DainoDB {
    /// Open or create the database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())
            .with_context(|| format!("Failed to create DB directory: {:?}", path.as_ref()))?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(MAX_DBS)
                .open(path.as_ref())
                .with_context(|| format!("Failed to open LMDB env at {:?}", path.as_ref()))?
        };

        let mut wtxn = env.write_txn()?;
        let blocks_by_height = env.create_database(&mut wtxn, Some("blocks_by_height"))?;
        let hash_to_height = env.create_database(&mut wtxn, Some("hash_to_height"))?;
        let txs_by_id = env.create_database(&mut wtxn, Some("txs_by_id"))?;
        let meta = env.create_database(&mut wtxn, Some("meta"))?;
        wtxn.commit()?;

        Ok(DainoDB {
            env,
            blocks_by_height,
            hash_to_height,
            txs_by_id,
            meta,
        })
    }

    /// Store a block record and its transactions in a single LMDB transaction.
    pub fn put_block(&self, block: &BlockRecord, txs: &[TxRecord]) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;

        // Store block by height
        let height_key = block.height.to_be_bytes();
        let block_bytes = bincode::serialize(block)?;
        self.blocks_by_height
            .put(&mut wtxn, &height_key, &block_bytes)?;

        // Store hash -> height mapping
        self.hash_to_height
            .put(&mut wtxn, &block.hash, &height_key)?;

        // Store each transaction
        for tx in txs {
            let tx_bytes = bincode::serialize(tx)?;
            self.txs_by_id.put(&mut wtxn, &tx.txid, &tx_bytes)?;
        }

        // Update chain metadata
        let meta = self.get_meta_inner(&wtxn)?.unwrap_or(ChainMeta {
            tip_height: 0,
            tip_hash: [0; 32],
            block_count: 0,
            tx_count: 0,
        });

        let new_meta = ChainMeta {
            tip_height: block.height,
            tip_hash: block.hash,
            block_count: meta.block_count + 1,
            tx_count: meta.tx_count + txs.len() as u64,
        };
        let meta_bytes = bincode::serialize(&new_meta)?;
        self.meta.put(&mut wtxn, "chain", &meta_bytes)?;

        wtxn.commit()?;
        Ok(())
    }

    /// Get a block record by height.
    pub fn get_block_by_height(&self, height: u32) -> Result<Option<BlockRecord>> {
        let rtxn = self.env.read_txn()?;
        let key = height.to_be_bytes();
        match self.blocks_by_height.get(&rtxn, &key)? {
            Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    /// Get a block record by hash (internal byte order).
    pub fn get_block_by_hash(&self, hash: &[u8; 32]) -> Result<Option<BlockRecord>> {
        let rtxn = self.env.read_txn()?;
        match self.hash_to_height.get(&rtxn, hash.as_slice())? {
            Some(height_bytes) => match self.blocks_by_height.get(&rtxn, height_bytes)? {
                Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    /// Get a transaction record by txid (internal byte order).
    pub fn get_tx(&self, txid: &[u8; 32]) -> Result<Option<TxRecord>> {
        let rtxn = self.env.read_txn()?;
        match self.txs_by_id.get(&rtxn, txid.as_slice())? {
            Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    /// Get the current chain metadata.
    pub fn get_meta(&self) -> Result<Option<ChainMeta>> {
        let rtxn = self.env.read_txn()?;
        self.get_meta_inner(&rtxn)
    }

    fn get_meta_inner(&self, txn: &heed::RoTxn) -> Result<Option<ChainMeta>> {
        match self.meta.get(txn, "chain")? {
            Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    /// Get the current chain tip height, or None if the database is empty.
    pub fn tip_height(&self) -> Result<Option<u32>> {
        Ok(self.get_meta()?.map(|m| m.tip_height))
    }

    /// Get a summary string for display.
    pub fn status_summary(&self) -> Result<String> {
        match self.get_meta()? {
            Some(meta) => Ok(format!(
                "tip={} hash={} blocks={} txs={}",
                meta.tip_height,
                hash_to_display(&meta.tip_hash),
                meta.block_count,
                meta.tx_count,
            )),
            None => Ok("empty database".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (tempfile::TempDir, DainoDB) {
        let dir = tempfile::tempdir().unwrap();
        let db = DainoDB::open(dir.path()).unwrap();
        (dir, db)
    }

    #[test]
    fn test_open_and_close() {
        let (_dir, db) = temp_db();
        assert!(db.get_meta().unwrap().is_none());
    }

    #[test]
    fn test_put_and_get_block() {
        let (_dir, db) = temp_db();

        let block = BlockRecord {
            height: 0,
            hash: [0xAA; 32],
            prev_hash: [0x00; 32],
            merkle_root: [0xBB; 32],
            time: 1317972665,
            bits: 0x1e0ffff0,
            nonce: 99943,
            tx_count: 1,
            size: 286,
        };

        let tx = TxRecord {
            txid: [0xCC; 32],
            block_height: 0,
            tx_index: 0,
            version: 1,
            tx_type: 0,
            lock_time: 0,
            value_out: 5000000000,
            input_count: 1,
            output_count: 1,
        };

        db.put_block(&block, &[tx.clone()]).unwrap();

        // Retrieve by height
        let got = db.get_block_by_height(0).unwrap().unwrap();
        assert_eq!(got.height, 0);
        assert_eq!(got.hash, [0xAA; 32]);
        assert_eq!(got.nonce, 99943);

        // Retrieve by hash
        let got = db.get_block_by_hash(&[0xAA; 32]).unwrap().unwrap();
        assert_eq!(got.height, 0);

        // Retrieve tx
        let got_tx = db.get_tx(&[0xCC; 32]).unwrap().unwrap();
        assert_eq!(got_tx.block_height, 0);
        assert_eq!(got_tx.value_out, 5000000000);

        // Check metadata
        let meta = db.get_meta().unwrap().unwrap();
        assert_eq!(meta.tip_height, 0);
        assert_eq!(meta.block_count, 1);
        assert_eq!(meta.tx_count, 1);
    }

    #[test]
    fn test_multiple_blocks() {
        let (_dir, db) = temp_db();

        for h in 0..10u32 {
            let mut hash = [0u8; 32];
            hash[0..4].copy_from_slice(&h.to_le_bytes());

            let block = BlockRecord {
                height: h,
                hash,
                prev_hash: if h == 0 {
                    [0; 32]
                } else {
                    let mut prev = [0u8; 32];
                    prev[0..4].copy_from_slice(&(h - 1).to_le_bytes());
                    prev
                },
                merkle_root: [0; 32],
                time: 1317972665 + h * 150,
                bits: 0x1e0ffff0,
                nonce: h,
                tx_count: 1,
                size: 286,
            };

            db.put_block(&block, &[]).unwrap();
        }

        let meta = db.get_meta().unwrap().unwrap();
        assert_eq!(meta.tip_height, 9);
        assert_eq!(meta.block_count, 10);

        // Spot check
        let b5 = db.get_block_by_height(5).unwrap().unwrap();
        assert_eq!(b5.nonce, 5);
    }

    #[test]
    fn test_missing_block() {
        let (_dir, db) = temp_db();
        assert!(db.get_block_by_height(999).unwrap().is_none());
        assert!(db.get_block_by_hash(&[0xFF; 32]).unwrap().is_none());
        assert!(db.get_tx(&[0xFF; 32]).unwrap().is_none());
    }
}
