//! ZMQ subscriber for real-time dashd notifications.
//!
//! Dashd publishes notifications via ZMQ when new blocks and transactions
//! are received. This module subscribes to those notifications and converts
//! them into a stream of events that the indexer can consume.
//!
//! ZMQ topics supported by dashd:
//! - `hashblock` -- 32-byte block hash when a new block is connected
//! - `hashtx` -- 32-byte txid when a new transaction enters the mempool
//! - `rawblock` -- full serialized block data
//! - `rawtx` -- full serialized transaction data
//! - `rawtxlocksig` -- InstantSend lock notification
//! - `rawchainlocksig` -- ChainLock notification
//!
//! Daino subscribes to `hashblock` to know when to fetch new blocks
//! via RPC, and optionally `hashtx` for mempool tracking.

use anyhow::Result;
use tokio::sync::mpsc;

/// Events emitted by the ZMQ subscriber.
#[derive(Debug, Clone)]
pub enum ZmqEvent {
    /// A new block hash was published (internal byte order).
    NewBlock([u8; 32]),
    /// A new transaction hash was published (internal byte order).
    NewTransaction([u8; 32]),
    /// A ChainLock signature was published (raw bytes).
    ChainLock(Vec<u8>),
    /// A raw block was published (full serialized block).
    RawBlock(Vec<u8>),
    /// A raw transaction was published (full serialized transaction).
    RawTransaction(Vec<u8>),
}

/// ZMQ subscriber configuration.
#[derive(Debug, Clone)]
pub struct ZmqConfig {
    /// ZMQ endpoint for hashblock notifications (e.g. "tcp://127.0.0.1:29998")
    pub hashblock_endpoint: Option<String>,
    /// ZMQ endpoint for hashtx notifications
    pub hashtx_endpoint: Option<String>,
    /// ZMQ endpoint for rawblock notifications
    pub rawblock_endpoint: Option<String>,
    /// ZMQ endpoint for rawtx notifications
    pub rawtx_endpoint: Option<String>,
    /// ZMQ endpoint for rawchainlocksig notifications
    pub chainlocksig_endpoint: Option<String>,
}

impl ZmqConfig {
    /// Create a config with all endpoints on the same address with standard ports.
    ///
    /// Dashd defaults: hashblock and hashtx on the same endpoint.
    pub fn from_endpoint(endpoint: &str) -> Self {
        ZmqConfig {
            hashblock_endpoint: Some(endpoint.to_string()),
            hashtx_endpoint: Some(endpoint.to_string()),
            rawblock_endpoint: None,
            rawtx_endpoint: None,
            chainlocksig_endpoint: None,
        }
    }

    /// Create a minimal config that only subscribes to hashblock.
    pub fn hashblock_only(endpoint: &str) -> Self {
        ZmqConfig {
            hashblock_endpoint: Some(endpoint.to_string()),
            hashtx_endpoint: None,
            rawblock_endpoint: None,
            rawtx_endpoint: None,
            chainlocksig_endpoint: None,
        }
    }
}

/// A ZMQ subscriber that connects to dashd and emits events.
///
/// This is a lightweight wrapper that spawns async tasks to read from
/// ZMQ sockets and forward events through a tokio mpsc channel.
///
/// # Usage
///
/// ```no_run
/// use daino_fetch::zmq::{ZmqConfig, ZmqSubscriber};
///
/// # async fn example() -> anyhow::Result<()> {
/// let config = ZmqConfig::hashblock_only("tcp://127.0.0.1:29998");
/// let (subscriber, mut rx) = ZmqSubscriber::start(config).await?;
///
/// while let Some(event) = rx.recv().await {
///     println!("Got event: {:?}", event);
/// }
/// # Ok(())
/// # }
/// ```
pub struct ZmqSubscriber {
    /// Handle to abort the subscriber tasks on drop.
    _handles: Vec<tokio::task::JoinHandle<()>>,
}

impl ZmqSubscriber {
    /// Start subscribing to ZMQ notifications.
    ///
    /// Returns the subscriber handle and a receiver channel for events.
    /// The subscriber runs in background tasks until dropped.
    pub async fn start(
        config: ZmqConfig,
    ) -> Result<(Self, mpsc::Receiver<ZmqEvent>)> {
        let (tx, rx) = mpsc::channel(256);
        let mut handles = Vec::new();

        // For each configured endpoint, spawn a subscriber task.
        // We use a simple polling approach: connect a raw TCP/ZMQ socket
        // and parse the multipart messages.
        //
        // ZMQ multipart message format from dashd:
        //   Frame 0: topic (e.g. "hashblock")
        //   Frame 1: body (e.g. 32-byte hash)
        //   Frame 2: sequence number (4-byte LE u32)

        if let Some(endpoint) = &config.hashblock_endpoint {
            let tx = tx.clone();
            let endpoint = endpoint.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = subscribe_loop(&endpoint, "hashblock", tx).await {
                    eprintln!("ZMQ hashblock subscriber error: {}", e);
                }
            });
            handles.push(handle);
        }

        if let Some(endpoint) = &config.hashtx_endpoint {
            let tx = tx.clone();
            let endpoint = endpoint.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = subscribe_loop(&endpoint, "hashtx", tx).await {
                    eprintln!("ZMQ hashtx subscriber error: {}", e);
                }
            });
            handles.push(handle);
        }

        if let Some(endpoint) = &config.chainlocksig_endpoint {
            let tx = tx.clone();
            let endpoint = endpoint.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = subscribe_loop(&endpoint, "rawchainlocksig", tx).await {
                    eprintln!("ZMQ chainlocksig subscriber error: {}", e);
                }
            });
            handles.push(handle);
        }

        if handles.is_empty() {
            anyhow::bail!("No ZMQ endpoints configured");
        }

        Ok((ZmqSubscriber { _handles: handles }, rx))
    }
}

/// Main subscribe loop for a single ZMQ topic.
///
/// NOTE: This is a placeholder implementation. A full implementation would
/// use a ZMQ library (like zeromq-rs or tmq). For now, we provide the
/// interface and event types so the rest of the system can be wired up.
/// The actual ZMQ socket handling will be added when we integrate with
/// a running dashd instance.
async fn subscribe_loop(
    endpoint: &str,
    topic: &str,
    _tx: mpsc::Sender<ZmqEvent>,
) -> Result<()> {
    // TODO: Replace with actual ZMQ subscription using zeromq or tmq crate.
    // For now, log that we would connect and return.
    //
    // The ZMQ protocol for dashd notifications:
    // 1. Connect SUB socket to endpoint
    // 2. Subscribe to topic prefix
    // 3. Receive multipart messages: [topic, body, sequence]
    // 4. Parse body based on topic and emit ZmqEvent

    println!(
        "ZMQ: would subscribe to '{}' on {} (not yet implemented)",
        topic, endpoint
    );

    // In the future, this loop will:
    // loop {
    //     let msg = socket.recv_multipart().await?;
    //     let topic_frame = &msg[0];
    //     let body = &msg[1];
    //     let event = match topic {
    //         "hashblock" => {
    //             let mut hash = [0u8; 32];
    //             hash.copy_from_slice(body);
    //             ZmqEvent::NewBlock(hash)
    //         }
    //         "hashtx" => { ... }
    //         _ => continue,
    //     };
    //     tx.send(event).await?;
    // }

    // For now, just sleep forever (the task will be cancelled on drop)
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zmq_config_from_endpoint() {
        let config = ZmqConfig::from_endpoint("tcp://127.0.0.1:29998");
        assert_eq!(
            config.hashblock_endpoint.as_deref(),
            Some("tcp://127.0.0.1:29998")
        );
        assert_eq!(
            config.hashtx_endpoint.as_deref(),
            Some("tcp://127.0.0.1:29998")
        );
        assert!(config.rawblock_endpoint.is_none());
    }

    #[test]
    fn test_zmq_config_hashblock_only() {
        let config = ZmqConfig::hashblock_only("tcp://127.0.0.1:29998");
        assert!(config.hashblock_endpoint.is_some());
        assert!(config.hashtx_endpoint.is_none());
    }

    #[test]
    fn test_zmq_event_debug() {
        let event = ZmqEvent::NewBlock([0xAA; 32]);
        let debug = format!("{:?}", event);
        assert!(debug.contains("NewBlock"));
    }
}
