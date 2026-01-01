//! Special transaction payloads

pub mod asset_lock;
pub mod coinbase;
pub mod provider_register;
pub mod provider_revoke;
pub mod provider_update_registrar;
pub mod provider_update_service;

pub use asset_lock::{AssetLockPayload, AssetUnlockPayload};
pub use coinbase::CbTx;
pub use provider_register::ProRegTx;
pub use provider_revoke::{ProUpRevTx, RevocationReason};
pub use provider_update_registrar::ProUpRegTx;
pub use provider_update_service::{MasternodeType, ProUpServTx};
