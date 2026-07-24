//! daino-state: Storage and indexing for the Daino blockchain indexer.
//!
//! Uses LMDB (via heed) for persistent storage of:
//! - Block index (height <-> hash, header data)
//! - Transaction index (txid -> block location)
//! - Address index (address -> transaction history)

pub mod apply;
pub mod db;

pub use apply::{build_block_batch, spent_addrs_from_utxos};
pub use daino_core::TxProvider;
pub use db::{BlockBatch, DainoDB};
