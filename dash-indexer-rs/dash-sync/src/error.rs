use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("ZMQ error: {0}")]
    Zmq(String),

    #[error("RPC error: {0}")]
    Rpc(#[from] bitcoincore_rpc::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Database error: {0}")]
    Database(#[from] dash_state::StateError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SyncError>;
