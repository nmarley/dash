pub mod error;
pub mod zmq;

pub use error::{Result, SyncError};
pub use zmq::ZmqConsumer;
