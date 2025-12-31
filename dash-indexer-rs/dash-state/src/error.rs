use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Database error: {0}")]
    Database(#[from] lmdb::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Transaction not found: {0}")]
    TxNotFound(String),

    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StateError>;
