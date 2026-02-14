//! daino-core: Shared types, traits, and block/undo file reading for Daino.
//!
//! This crate provides:
//! - Block file reading (`blk*.dat`)
//! - Undo file reading (`rev*.dat`)
//! - Common types used across the Daino workspace

pub mod block_reader;
pub mod undo;
pub mod undo_reader;

pub use block_reader::{BlockFileReader, Network};
pub use undo::CBlockUndo;
pub use undo_reader::UndoFileReader;
