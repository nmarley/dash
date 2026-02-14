//! daino-state: Storage and indexing for the Daino blockchain indexer.
//!
//! Uses LMDB (via heed) for persistent storage of:
//! - Block index (height <-> hash, header data)
//! - Transaction index (txid -> block location)
//! - Address index (address -> transaction history)

pub mod db;

pub use db::DainoDB;
