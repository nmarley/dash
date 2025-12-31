// Copyright (c) 2025 The Dash Core developers
// Distributed under the MIT software license

//! # librustdash
//!
//! Rust primitives library for Dash blockchain data structures.
//!
//! This library provides types and serialization for Dash blocks, transactions,
//! and special transaction payloads, with byte-for-byte compatibility with Dash Core.

pub mod error;
pub mod serialize;
pub mod transaction;
pub mod tx_type;

pub use error::{DashError, Result};
pub use transaction::{OutPoint, TxIn, TxOut, Transaction};
pub use tx_type::DashTxType;
