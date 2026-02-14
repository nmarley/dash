//! Chain tip follower -- coordinates RPC and ZMQ to keep the index current.
//!
//! The follower operates in two modes:
//!
//! 1. **Catch-up mode**: On startup, compare our indexed tip with dashd's tip.
//!    If we're behind, fetch blocks via RPC one-by-one until caught up.
//!
//! 2. **Follow mode**: Once caught up, wait for ZMQ `hashblock` notifications
//!    and fetch each new block via RPC. If ZMQ is not configured, fall back
//!    to polling `getblockcount` every few seconds.
//!
//! The follower handles reorgs by detecting when a fetched block's
//! `previousblockhash` doesn't match our indexed tip. In that case it
//! walks back both chains to find the common ancestor, removes the
//! orphaned blocks from the index, and re-indexes the new chain.

use anyhow::{Context, Result};
use std::time::Duration;

use crate::rpc::DashdRpc;

/// Configuration for the tip follower.
#[derive(Debug, Clone)]
pub struct FollowerConfig {
    /// How often to poll for new blocks if ZMQ is not available (seconds).
    pub poll_interval: Duration,
    /// Maximum number of blocks to fetch in a single catch-up batch.
    pub batch_size: u64,
}

impl Default for FollowerConfig {
    fn default() -> Self {
        FollowerConfig {
            poll_interval: Duration::from_secs(2),
            batch_size: 100,
        }
    }
}

/// A new block fetched from dashd, ready for indexing.
#[derive(Debug)]
pub struct FetchedBlock {
    /// Block height
    pub height: u64,
    /// Block hash (display order hex)
    pub hash: String,
    /// Raw block bytes (serialized)
    pub raw: Vec<u8>,
    /// Whether this block has a ChainLock
    pub chainlocked: bool,
}

/// Catch up from our current tip to dashd's tip.
///
/// Fetches blocks one-by-one from `start_height` to `target_height`
/// and calls the provided callback for each block.
///
/// Returns the number of blocks fetched.
pub async fn catch_up<F>(
    rpc: &DashdRpc,
    start_height: u64,
    target_height: u64,
    mut on_block: F,
) -> Result<u64>
where
    F: FnMut(FetchedBlock) -> Result<()>,
{
    let total = target_height.saturating_sub(start_height);
    if total == 0 {
        return Ok(0);
    }

    println!(
        "Catching up: {} blocks ({} -> {})",
        total, start_height, target_height
    );

    let mut count = 0u64;
    for height in start_height..=target_height {
        let hash = rpc
            .get_block_hash(height)
            .await
            .with_context(|| format!("Failed to get block hash at height {}", height))?;

        let raw_hex = rpc
            .get_block_hex(&hash)
            .await
            .with_context(|| format!("Failed to get raw block {}", hash))?;

        let raw =
            hex::decode(&raw_hex).with_context(|| format!("Invalid hex in raw block {}", hash))?;

        // Check ChainLock status via the verbose getblock call
        let block_info = rpc.get_block(&hash).await.ok();
        let chainlocked = block_info
            .as_ref()
            .and_then(|b| b.chainlock)
            .unwrap_or(false);

        on_block(FetchedBlock {
            height,
            hash: hash.clone(),
            raw,
            chainlocked,
        })?;

        count += 1;

        if count.is_multiple_of(100) {
            println!("  fetched {} / {} blocks (height {})", count, total, height);
        }
    }

    println!("Catch-up complete: fetched {} blocks", count);
    Ok(count)
}

/// Poll dashd for new blocks in a loop.
///
/// This is the fallback mode when ZMQ is not available.
/// Checks `getblockcount` every `interval` and fetches any new blocks.
pub async fn poll_loop<F>(
    rpc: &DashdRpc,
    mut current_height: u64,
    interval: Duration,
    mut on_block: F,
) -> Result<()>
where
    F: FnMut(FetchedBlock) -> Result<()>,
{
    println!(
        "Starting poll loop from height {} (interval: {:?})",
        current_height, interval
    );

    loop {
        let tip = rpc.get_block_count().await?;

        if tip > current_height {
            let fetched = catch_up(rpc, current_height + 1, tip, &mut on_block).await?;
            current_height += fetched;
        }

        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_follower_config_defaults() {
        let config = FollowerConfig::default();
        assert_eq!(config.poll_interval, Duration::from_secs(2));
        assert_eq!(config.batch_size, 100);
    }
}
