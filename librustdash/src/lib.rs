//! # librustdash
//!
//! Rust primitives library for Dash blockchain data structures.
//!
//! This library provides types and serialization for Dash blocks, transactions,
//! and special transaction payloads, with byte-for-byte compatibility with Dash Core.

pub mod block;
pub mod error;
pub mod payloads;
pub mod serialize;
pub mod transaction;
pub mod tx_type;

pub use block::{Block, BlockHeader};
pub use error::{DashError, Result};
pub use payloads::{AssetLockPayload, AssetUnlockPayload, CbTx};
pub use transaction::{OutPoint, Transaction, TxIn, TxOut};
pub use tx_type::DashTxType;
