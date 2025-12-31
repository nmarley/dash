//! Error types for librustdash

use thiserror::Error;

/// Errors that can occur when working with Dash primitives
#[derive(Debug, Error)]
pub enum DashError {
    #[error("Invalid transaction version: {0}")]
    InvalidVersion(u16),

    #[error("Unknown transaction type: {0}")]
    UnknownTxType(u16),

    #[error("Invalid payload for transaction type")]
    InvalidPayload,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid BLS signature")]
    InvalidBlsSignature,

    #[error("Invalid BLS public key")]
    InvalidBlsPublicKey,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),
}

/// Result type alias for librustdash operations
pub type Result<T> = std::result::Result<T, DashError>;
