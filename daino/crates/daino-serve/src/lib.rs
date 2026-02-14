//! daino-serve: REST API server for the Daino blockchain indexer.
//!
//! Provides Insight-compatible REST endpoints for querying
//! blocks, transactions, and chain status.

pub mod api;
pub mod server;

pub use server::start_server;
