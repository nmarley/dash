//! LMDB-backed database for Daino blockchain indexer.
//!
//! Stores and retrieves:
//! - Blocks by height and hash
//! - Transactions by txid
//! - Address history (address -> txids)
//! - Chain tip metadata
//!
//! Uses heed (safe LMDB wrapper) for crash-safe, memory-mapped storage.

use std::path::Path;

use anyhow::{bail, Context, Result};
use heed::types::*;
use heed::{CompactionOption, Database, Env, EnvOpenOptions};
use librustdash::hash::hash_to_display;

/// A stored block record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockRecord {
    /// Block height
    pub height: u32,
    /// Block hash (internal byte order)
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

/// A stored transaction record.
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

/// A reference from an address to a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrTxRef {
    /// Block height (for ordering)
    pub block_height: u32,
    /// Transaction ID (internal byte order)
    pub txid: [u8; 32],
}

/// An unspent transaction output.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UtxoEntry {
    /// Transaction ID (internal byte order)
    pub txid: [u8; 32],
    /// Output index within the transaction
    pub vout: u32,
    /// Value in satoshis
    pub value: i64,
    /// Block height where this output was created
    pub block_height: u32,
    /// The 20-byte address hash (if standard script)
    pub addr_hash: Option<[u8; 20]>,
}

/// A spent outpoint (txid + vout) -- used to remove UTXOs when inputs consume them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpentOutpoint {
    /// Transaction ID of the output being spent (internal byte order)
    pub txid: [u8; 32],
    /// Output index being spent
    pub vout: u32,
}

/// Internal storage format for UTXO values.
///
/// The outpoint key already encodes txid + vout, so we only store the
/// non-key fields in the value. This saves ~38% vs storing the full
/// `UtxoEntry` (33 bytes vs ~75 bytes per UTXO).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct UtxoValue {
    value: i64,
    block_height: u32,
    addr_hash: Option<[u8; 20]>,
}

/// Outpoint key: txid(32) + vout(4 BE) = 36 bytes.
const OUTPOINT_KEY_LEN: usize = 32 + 4;

fn make_outpoint_key(txid: &[u8; 32], vout: u32) -> [u8; OUTPOINT_KEY_LEN] {
    let mut key = [0u8; OUTPOINT_KEY_LEN];
    key[0..32].copy_from_slice(txid);
    key[32..36].copy_from_slice(&vout.to_be_bytes());
    key
}

/// Extract txid and vout from an outpoint key.
#[allow(dead_code)]
fn parse_outpoint_key(key: &[u8]) -> ([u8; 32], u32) {
    let mut txid = [0u8; 32];
    txid.copy_from_slice(&key[0..32]);
    let mut vout_bytes = [0u8; 4];
    vout_bytes.copy_from_slice(&key[32..36]);
    (txid, u32::from_be_bytes(vout_bytes))
}

/// Address UTXO key: addr_hash(20) + txid(32) + vout(4 BE) = 56 bytes.
/// Prefix scan on addr_hash returns all UTXOs for that address.
const ADDR_UTXO_KEY_LEN: usize = 20 + 32 + 4;

fn make_addr_utxo_key(addr_hash: &[u8; 20], txid: &[u8; 32], vout: u32) -> [u8; ADDR_UTXO_KEY_LEN] {
    let mut key = [0u8; ADDR_UTXO_KEY_LEN];
    key[0..20].copy_from_slice(addr_hash);
    key[20..52].copy_from_slice(txid);
    key[52..56].copy_from_slice(&vout.to_be_bytes());
    key
}

/// Address index key: addr_hash(20) + block_height(4 BE) + txid(32) = 56 bytes.
/// This compound key allows prefix scanning by addr_hash to get all txs,
/// naturally ordered by block height.
const ADDR_KEY_LEN: usize = 20 + 4 + 32;

fn make_addr_key(addr_hash: &[u8; 20], height: u32, txid: &[u8; 32]) -> [u8; ADDR_KEY_LEN] {
    let mut key = [0u8; ADDR_KEY_LEN];
    key[0..20].copy_from_slice(addr_hash);
    key[20..24].copy_from_slice(&height.to_be_bytes());
    key[24..56].copy_from_slice(txid);
    key
}

fn parse_addr_key(key: &[u8]) -> Option<([u8; 20], AddrTxRef)> {
    if key.len() < ADDR_KEY_LEN {
        return None;
    }
    let mut addr_hash = [0u8; 20];
    addr_hash.copy_from_slice(&key[0..20]);
    let mut height_bytes = [0u8; 4];
    height_bytes.copy_from_slice(&key[20..24]);
    let mut txid = [0u8; 32];
    txid.copy_from_slice(&key[24..56]);
    Some((
        addr_hash,
        AddrTxRef {
            block_height: u32::from_be_bytes(height_bytes),
            txid,
        },
    ))
}

/// A batch of data for one block, used with `DainoDB::put_batch()`.
#[derive(Debug, Clone)]
pub struct BlockBatch {
    pub block: BlockRecord,
    pub txs: Vec<TxRecord>,
    pub addr_refs: Vec<([u8; 20], AddrTxRef)>,
    pub new_utxos: Vec<UtxoEntry>,
    pub spent: Vec<SpentOutpoint>,
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
    /// compound key: addr_hash(20)+height(4)+txid(32) -> empty
    /// prefix scan on addr_hash(20) returns all txs for that address
    addr_to_txs: Database<Bytes, Bytes>,
    /// outpoint key: txid(32)+vout(4) -> UtxoEntry (bincode)
    utxos: Database<Bytes, Bytes>,
    /// addr UTXO key: addr_hash(20)+txid(32)+vout(4) -> empty
    /// prefix scan on addr_hash(20) yields all UTXOs for that address
    addr_utxos: Database<Bytes, Bytes>,
    /// "meta" -> ChainMeta (bincode)
    meta: Database<Str, Bytes>,
}

/// Maximum database size: 10 GB.
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
        let addr_to_txs = env.create_database(&mut wtxn, Some("addr_to_txs"))?;
        let utxos = env.create_database(&mut wtxn, Some("utxos"))?;
        let addr_utxos = env.create_database(&mut wtxn, Some("addr_utxos"))?;
        let meta = env.create_database(&mut wtxn, Some("meta"))?;
        wtxn.commit()?;

        Ok(DainoDB {
            env,
            blocks_by_height,
            hash_to_height,
            txs_by_id,
            addr_to_txs,
            utxos,
            addr_utxos,
            meta,
        })
    }

    /// A batch of block data to write in a single LMDB transaction.
    ///
    /// Batching many blocks into one write transaction avoids per-block
    /// fsync overhead, which is the main bottleneck during bulk indexing.

    /// Store a single block (convenience wrapper around `put_batch`).
    pub fn put_block(
        &self,
        block: &BlockRecord,
        txs: &[TxRecord],
        addr_refs: &[([u8; 20], AddrTxRef)],
        new_utxos: &[UtxoEntry],
        spent: &[SpentOutpoint],
    ) -> Result<()> {
        self.put_batch(&[BlockBatch {
            block: block.clone(),
            txs: txs.to_vec(),
            addr_refs: addr_refs.to_vec(),
            new_utxos: new_utxos.to_vec(),
            spent: spent.to_vec(),
        }])
    }

    /// Store multiple blocks in a single LMDB write transaction.
    ///
    /// This is the primary write method for bulk indexing. Writing N blocks
    /// in one transaction means only one fsync instead of N, which can be
    /// 10-50x faster for large batches.
    ///
    /// Blocks must be provided in height order.
    pub fn put_batch(&self, blocks: &[BlockBatch]) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }

        let mut wtxn = self.env.write_txn()?;

        let mut total_new_txs: u64 = 0;

        for batch in blocks {
            let block = &batch.block;

            // Block by height
            let height_key = block.height.to_be_bytes();
            let block_bytes = bincode::serialize(block)?;
            self.blocks_by_height
                .put(&mut wtxn, &height_key, &block_bytes)?;

            // Hash -> height
            self.hash_to_height
                .put(&mut wtxn, &block.hash, &height_key)?;

            // Transactions
            for tx in &batch.txs {
                let tx_bytes = bincode::serialize(tx)?;
                self.txs_by_id.put(&mut wtxn, &tx.txid, &tx_bytes)?;
            }

            // Address index
            for (addr_hash, tx_ref) in &batch.addr_refs {
                let key = make_addr_key(addr_hash, tx_ref.block_height, &tx_ref.txid);
                self.addr_to_txs.put(&mut wtxn, &key, &[])?;
            }

            // Remove spent UTXOs
            for outpoint in &batch.spent {
                let key = make_outpoint_key(&outpoint.txid, outpoint.vout);
                if let Some(val_bytes) = self.utxos.get(&wtxn, &key)?
                    && let Ok(val) = bincode::deserialize::<UtxoValue>(val_bytes)
                    && let Some(addr_hash) = &val.addr_hash
                {
                    let addr_key =
                        make_addr_utxo_key(addr_hash, &outpoint.txid, outpoint.vout);
                    self.addr_utxos.delete(&mut wtxn, &addr_key)?;
                }
                self.utxos.delete(&mut wtxn, &key)?;
            }

            // Add new UTXOs (store only non-key fields)
            for utxo in &batch.new_utxos {
                let key = make_outpoint_key(&utxo.txid, utxo.vout);
                let val = UtxoValue {
                    value: utxo.value,
                    block_height: utxo.block_height,
                    addr_hash: utxo.addr_hash,
                };
                let val_bytes = bincode::serialize(&val)?;
                self.utxos.put(&mut wtxn, &key, &val_bytes)?;

                if let Some(addr_hash) = &utxo.addr_hash {
                    let addr_key = make_addr_utxo_key(addr_hash, &utxo.txid, utxo.vout);
                    self.addr_utxos.put(&mut wtxn, &addr_key, &[])?;
                }
            }

            total_new_txs += batch.txs.len() as u64;
        }

        // Update metadata once for the entire batch
        let last = &blocks[blocks.len() - 1].block;
        let meta = self.get_meta_inner(&wtxn)?.unwrap_or(ChainMeta {
            tip_height: 0,
            tip_hash: [0; 32],
            block_count: 0,
            tx_count: 0,
        });

        let new_meta = ChainMeta {
            tip_height: last.height,
            tip_hash: last.hash,
            block_count: meta.block_count + blocks.len() as u32,
            tx_count: meta.tx_count + total_new_txs,
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

    /// Get all transaction references for an address (by 20-byte hash).
    ///
    /// Returns txids in block height order (oldest first).
    pub fn get_addr_txs(&self, addr_hash: &[u8; 20]) -> Result<Vec<AddrTxRef>> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::new();

        // Prefix scan: iterate all keys starting with addr_hash(20 bytes)
        let prefix = addr_hash.as_slice();
        let iter = self.addr_to_txs.prefix_iter(&rtxn, prefix)?;

        for item in iter {
            let (key, _value) = item?;
            if let Some((_addr, tx_ref)) = parse_addr_key(key) {
                results.push(tx_ref);
            }
        }

        Ok(results)
    }

    /// Get all unspent outputs for an address (by 20-byte hash).
    ///
    /// Returns UTXOs ordered by outpoint (txid + vout).
    pub fn get_addr_utxos(&self, addr_hash: &[u8; 20]) -> Result<Vec<UtxoEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::new();

        // Prefix scan on addr_utxos: keys start with addr_hash(20)
        let prefix = addr_hash.as_slice();
        let iter = self.addr_utxos.prefix_iter(&rtxn, prefix)?;

        for item in iter {
            let (key, _value) = item?;
            if key.len() < ADDR_UTXO_KEY_LEN {
                continue;
            }
            // Extract txid + vout from the key to look up the UTXO
            let mut txid = [0u8; 32];
            txid.copy_from_slice(&key[20..52]);
            let mut vout_bytes = [0u8; 4];
            vout_bytes.copy_from_slice(&key[52..56]);
            let vout = u32::from_be_bytes(vout_bytes);

            let outpoint_key = make_outpoint_key(&txid, vout);
            if let Some(val_bytes) = self.utxos.get(&rtxn, &outpoint_key)?
                && let Ok(val) = bincode::deserialize::<UtxoValue>(val_bytes)
            {
                results.push(UtxoEntry {
                    txid,
                    vout,
                    value: val.value,
                    block_height: val.block_height,
                    addr_hash: val.addr_hash,
                });
            }
        }

        Ok(results)
    }

    /// Get a single UTXO by outpoint (txid + vout).
    pub fn get_utxo(&self, txid: &[u8; 32], vout: u32) -> Result<Option<UtxoEntry>> {
        let rtxn = self.env.read_txn()?;
        let key = make_outpoint_key(txid, vout);
        match self.utxos.get(&rtxn, &key)? {
            Some(bytes) => {
                let val: UtxoValue = bincode::deserialize(bytes)?;
                Ok(Some(UtxoEntry {
                    txid: *txid,
                    vout,
                    value: val.value,
                    block_height: val.block_height,
                    addr_hash: val.addr_hash,
                }))
            }
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

    /// Return the on-disk size of the LMDB data file in bytes.
    pub fn real_disk_size(&self) -> Result<u64> {
        Ok(self.env.real_disk_size()?)
    }

    /// Compact the database by copying it to `dest` with dead pages omitted.
    ///
    /// The destination path must NOT already exist. LMDB's `mdb_env_copyfd2`
    /// with `MDB_CP_COMPACT` sequentially renumbers all live pages, reclaiming
    /// space from deleted entries (spent UTXOs, etc.).
    pub fn compact(&self, dest: &Path) -> Result<()> {
        if dest.exists() {
            bail!("Destination already exists: {:?}", dest);
        }
        self.env
            .copy_to_file(dest, CompactionOption::Enabled)
            .with_context(|| {
                format!("Failed to compact database to {:?}", dest)
            })?;
        Ok(())
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

        db.put_block(&block, std::slice::from_ref(&tx), &[], &[], &[])
            .unwrap();

        let got = db.get_block_by_height(0).unwrap().unwrap();
        assert_eq!(got.height, 0);
        assert_eq!(got.hash, [0xAA; 32]);
        assert_eq!(got.nonce, 99943);

        let got = db.get_block_by_hash(&[0xAA; 32]).unwrap().unwrap();
        assert_eq!(got.height, 0);

        let got_tx = db.get_tx(&[0xCC; 32]).unwrap().unwrap();
        assert_eq!(got_tx.block_height, 0);
        assert_eq!(got_tx.value_out, 5000000000);

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

            db.put_block(&block, &[], &[], &[], &[]).unwrap();
        }

        let meta = db.get_meta().unwrap().unwrap();
        assert_eq!(meta.tip_height, 9);
        assert_eq!(meta.block_count, 10);

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

    #[test]
    fn test_address_index() {
        let (_dir, db) = temp_db();

        let addr = [0x11u8; 20];
        let txid1 = [0xAA; 32];
        let txid2 = [0xBB; 32];
        let txid3 = [0xCC; 32];

        // Different address
        let other_addr = [0x22u8; 20];
        let txid4 = [0xDD; 32];

        let block = BlockRecord {
            height: 100,
            hash: [0x01; 32],
            prev_hash: [0x00; 32],
            merkle_root: [0; 32],
            time: 0,
            bits: 0,
            nonce: 0,
            tx_count: 3,
            size: 0,
        };

        let addr_refs = vec![
            (
                addr,
                AddrTxRef {
                    block_height: 100,
                    txid: txid1,
                },
            ),
            (
                addr,
                AddrTxRef {
                    block_height: 100,
                    txid: txid2,
                },
            ),
            (
                other_addr,
                AddrTxRef {
                    block_height: 100,
                    txid: txid4,
                },
            ),
        ];

        db.put_block(&block, &[], &addr_refs, &[], &[]).unwrap();

        // Second block with another tx for the same address
        let block2 = BlockRecord {
            height: 200,
            hash: [0x02; 32],
            prev_hash: [0x01; 32],
            merkle_root: [0; 32],
            time: 0,
            bits: 0,
            nonce: 0,
            tx_count: 1,
            size: 0,
        };

        let addr_refs2 = vec![(
            addr,
            AddrTxRef {
                block_height: 200,
                txid: txid3,
            },
        )];

        db.put_block(&block2, &[], &addr_refs2, &[], &[]).unwrap();

        // Query: addr should have 3 txs
        let txs = db.get_addr_txs(&addr).unwrap();
        assert_eq!(txs.len(), 3);
        assert_eq!(txs[0].block_height, 100);
        assert_eq!(txs[0].txid, txid1);
        assert_eq!(txs[1].block_height, 100);
        assert_eq!(txs[1].txid, txid2);
        assert_eq!(txs[2].block_height, 200);
        assert_eq!(txs[2].txid, txid3);

        // Query: other_addr should have 1 tx
        let txs = db.get_addr_txs(&other_addr).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].txid, txid4);

        // Query: unknown address should have 0
        let txs = db.get_addr_txs(&[0xFF; 20]).unwrap();
        assert_eq!(txs.len(), 0);
    }

    #[test]
    fn test_utxo_tracking() {
        let (_dir, db) = temp_db();

        let addr = [0x11u8; 20];
        let txid1 = [0xAA; 32];
        let txid2 = [0xBB; 32];

        // Block 1: creates two UTXOs for the same address
        let block1 = BlockRecord {
            height: 100,
            hash: [0x01; 32],
            prev_hash: [0x00; 32],
            merkle_root: [0; 32],
            time: 0,
            bits: 0,
            nonce: 0,
            tx_count: 1,
            size: 0,
        };

        let utxos = vec![
            UtxoEntry {
                txid: txid1,
                vout: 0,
                value: 500_000_000,
                block_height: 100,
                addr_hash: Some(addr),
            },
            UtxoEntry {
                txid: txid1,
                vout: 1,
                value: 300_000_000,
                block_height: 100,
                addr_hash: Some(addr),
            },
        ];

        db.put_block(&block1, &[], &[], &utxos, &[]).unwrap();

        // Should have 2 UTXOs
        let result = db.get_addr_utxos(&addr).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value, 500_000_000);
        assert_eq!(result[1].value, 300_000_000);

        // Look up individual UTXO
        let u = db.get_utxo(&txid1, 0).unwrap().unwrap();
        assert_eq!(u.value, 500_000_000);

        // Block 2: spends one UTXO (txid1:0) and creates a new one
        let block2 = BlockRecord {
            height: 200,
            hash: [0x02; 32],
            prev_hash: [0x01; 32],
            merkle_root: [0; 32],
            time: 0,
            bits: 0,
            nonce: 0,
            tx_count: 1,
            size: 0,
        };

        let new_utxos = vec![UtxoEntry {
            txid: txid2,
            vout: 0,
            value: 400_000_000,
            block_height: 200,
            addr_hash: Some(addr),
        }];

        let spent = vec![SpentOutpoint {
            txid: txid1,
            vout: 0,
        }];

        db.put_block(&block2, &[], &[], &new_utxos, &spent).unwrap();

        // Should now have 2 UTXOs: txid1:1 (unspent) and txid2:0 (new)
        let result = db.get_addr_utxos(&addr).unwrap();
        assert_eq!(result.len(), 2);

        // txid1:0 should be gone
        assert!(db.get_utxo(&txid1, 0).unwrap().is_none());

        // txid1:1 should still be there
        let u = db.get_utxo(&txid1, 1).unwrap().unwrap();
        assert_eq!(u.value, 300_000_000);

        // txid2:0 should exist
        let u = db.get_utxo(&txid2, 0).unwrap().unwrap();
        assert_eq!(u.value, 400_000_000);

        // Unknown address should have no UTXOs
        assert_eq!(db.get_addr_utxos(&[0xFF; 20]).unwrap().len(), 0);
    }
}
