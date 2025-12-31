// Copyright (c) 2025 The Dash Core developers
// Distributed under the MIT software license

//! Special transaction payloads

pub mod coinbase;
pub mod asset_lock;

pub use coinbase::CbTx;
pub use asset_lock::{AssetLockPayload, AssetUnlockPayload};
