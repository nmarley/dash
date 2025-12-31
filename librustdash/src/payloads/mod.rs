//! Special transaction payloads

pub mod asset_lock;
pub mod coinbase;

pub use asset_lock::{AssetLockPayload, AssetUnlockPayload};
pub use coinbase::CbTx;
