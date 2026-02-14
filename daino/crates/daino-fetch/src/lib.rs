//! # daino-fetch
//!
//! Communication layer between Daino and a running dashd instance.
//!
//! Provides:
//! - JSON-RPC client for querying dashd
//! - ZMQ subscriber for real-time block/tx notifications
//! - Chain tip follower that coordinates both for continuous indexing

pub mod follower;
pub mod rpc;
pub mod zmq;

pub use follower::{FetchedBlock, FollowerConfig, catch_up, poll_loop};
pub use rpc::DashdRpc;
pub use zmq::{ZmqConfig, ZmqEvent, ZmqSubscriber};
