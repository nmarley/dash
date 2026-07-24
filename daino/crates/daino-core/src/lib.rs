//! daino-core: Shared types, traits, and block/undo file reading for Daino.
//!
//! This crate provides:
//! - Block file reading (`blk*.dat`)
//! - Undo file reading (`rev*.dat`)
//! - Common types and provider traits used across the Daino workspace

pub mod block_reader;
pub mod difficulty;
pub mod provider;
pub mod undo;
pub mod undo_reader;

pub use block_reader::{BlockFileReader, Network, ScannedHeader};
pub use difficulty::{
    add_u256, difficulty_from_bits, target_from_bits, u256_to_hex, work_from_bits,
};
pub use provider::TxProvider;
pub use undo::CBlockUndo;
pub use undo_reader::{UndoFileReader, scan_undo_offsets};
