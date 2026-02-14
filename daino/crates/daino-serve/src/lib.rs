//! daino-serve: REST API server for the Daino blockchain indexer.
//!
//! Provides Insight-compatible REST endpoints for querying
//! blocks, transactions, addresses, and Dash-specific features
//! (ChainLocks, InstantSend, sporks, governance).

pub mod api;
pub mod server;

pub use server::{RpcConfig, start_server};
