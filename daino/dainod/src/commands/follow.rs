//! The `follow` subcommand -- connect to a running dashd and continuously index new blocks.
//!
//! This command:
//! 1. Connects to dashd via JSON-RPC
//! 2. Reconciles reorgs (disconnect to common ancestor when tip diverges)
//! 3. Catches up by fetching blocks via RPC
//! 4. Enters a poll loop to index new blocks as they arrive
//!
//! Each block fetched via RPC as raw hex is deserialized using librustdash,
//! then indexed through the same `build_block_batch` path as file indexing.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use librustdash::Block;
use librustdash::hash::hash_to_display;

use daino_core::{add_u256, work_from_bits};
use daino_fetch::rpc::DashdRpc;
use daino_fetch::{FetchedBlock, catch_up};
use daino_state::db::DainoDB;
use daino_state::{build_block_batch, spent_addrs_from_utxos};

/// Follow a running dashd instance and continuously index new blocks.
pub async fn follow_dashd(
    dbdir: &Path,
    rpc_url: &str,
    rpc_user: &str,
    rpc_password: &str,
    poll_interval: u64,
) -> Result<()> {
    let db = DainoDB::open(dbdir)?;

    let rpc = DashdRpc::new(rpc_url, rpc_user, rpc_password);

    println!("Connecting to dashd at {} ...", rpc_url);
    let info = rpc
        .get_blockchain_info()
        .await
        .context("Failed to connect to dashd. Is it running?")?;

    println!(
        "Connected: chain={} blocks={} headers={} progress={:.4}%",
        info.chain,
        info.blocks,
        info.headers,
        info.verification_progress * 100.0,
    );

    if info.initial_block_download {
        println!("WARNING: dashd is still performing initial block download.");
        println!("         Indexing will proceed but may be incomplete.");
    }

    let (start, dashd_tip) = reconcile_with_dashd(&db, &rpc).await?;
    println!(
        "Our tip: {:?}, dashd tip: {}, catch-up start: {}",
        db.tip_height()?,
        dashd_tip,
        start
    );

    if start <= dashd_tip {
        catch_up(&rpc, start, dashd_tip, |fetched| {
            index_fetched_block(&db, &fetched)
        })
        .await?;
    }

    println!();
    println!(
        "Caught up to tip {}. Entering follow mode (poll every {}s)...",
        db.tip_height()?.unwrap_or(0),
        poll_interval
    );
    println!("Press Ctrl-C to stop.");

    let interval = Duration::from_secs(poll_interval);
    loop {
        let (start, dashd_tip) = reconcile_with_dashd(&db, &rpc).await?;
        if start <= dashd_tip {
            catch_up(&rpc, start, dashd_tip, |fetched| {
                index_fetched_block(&db, &fetched)
            })
            .await?;
        }
        tokio::time::sleep(interval).await;
    }
}

/// Align local tip with dashd: disconnect through any reorg, return
/// `(next_height_to_fetch, dashd_tip)`.
async fn reconcile_with_dashd(db: &DainoDB, rpc: &DashdRpc) -> Result<(u64, u64)> {
    let dashd_tip = rpc.get_block_count().await?;

    if db.get_meta()?.is_none() {
        return Ok((0, dashd_tip));
    }

    // If we are ahead of dashd (rare), disconnect down to dashd's tip height.
    if let Some(our_tip) = db.tip_height()?
        && our_tip as u64 > dashd_tip
    {
        let removed = db.disconnect_to_height(dashd_tip as u32)?;
        println!("Local tip ahead of dashd: disconnected {removed} blocks to height {dashd_tip}");
    }

    // If tip hash diverges from dashd at the same height, walk back to
    // the common ancestor and disconnect the orphaned branch.
    if let Some(meta) = db.get_meta()? {
        let remote = rpc
            .get_block_hash(meta.tip_height as u64)
            .await
            .with_context(|| {
                format!(
                    "Failed to get dashd block hash at local tip height {}",
                    meta.tip_height
                )
            })?;
        let local = hash_to_display(&meta.tip_hash);
        if local != remote {
            let ancestor = find_common_ancestor(db, rpc, meta.tip_height).await?;
            let removed = db.disconnect_to_height(ancestor)?;
            println!(
                "Reorg detected: disconnected {removed} block(s) to common ancestor height {ancestor}"
            );
        }
    }

    let start = match db.tip_height()? {
        None => 0,
        Some(h) => h as u64 + 1,
    };
    Ok((start, dashd_tip))
}

/// Walk backward from `height` until local block hash matches dashd.
async fn find_common_ancestor(db: &DainoDB, rpc: &DashdRpc, mut height: u32) -> Result<u32> {
    loop {
        let local = db
            .get_block_by_height(height)?
            .ok_or_else(|| anyhow::anyhow!("missing local block at height {height}"))?;
        let local_hex = hash_to_display(&local.hash);

        match rpc.get_block_hash(height as u64).await {
            Ok(remote) if remote == local_hex => return Ok(height),
            Ok(_) | Err(_) => {
                if height == 0 {
                    bail!("no common ancestor with dashd (diverged from genesis)");
                }
                height -= 1;
            }
        }
    }
}

/// Deserialize a raw block and index it into the database via the shared apply path.
fn index_fetched_block(db: &DainoDB, fetched: &FetchedBlock) -> Result<()> {
    let block = Block::deserialize(&fetched.raw)
        .with_context(|| format!("Failed to deserialize block at height {}", fetched.height))?;

    let block_hash = block.header.block_hash()?;
    let height = fetched.height as u32;

    // Idempotent: already have this exact block.
    if let Some(existing) = db.get_block_by_height(height)?
        && existing.hash == block_hash
    {
        return Ok(());
    }

    // Must connect onto current tip.
    match db.get_meta()? {
        None => {
            if height != 0 {
                bail!("refusing non-genesis block at height {height} on empty database");
            }
        }
        Some(meta) => {
            if height != meta.tip_height + 1 {
                bail!(
                    "block height {height} does not extend tip {}",
                    meta.tip_height
                );
            }
            if block.header.prev_blockhash != meta.tip_hash {
                bail!(
                    "block prev hash mismatch at height {height}: expected {}, got {}",
                    hash_to_display(&meta.tip_hash),
                    hash_to_display(&block.header.prev_blockhash)
                );
            }
        }
    }

    let block_size = fetched.raw.len() as u32;

    let prev_chainwork = if height > 0 {
        db.get_block_by_height(height - 1)?
            .map(|b| b.chainwork)
            .unwrap_or([0u8; 32])
    } else {
        [0u8; 32]
    };
    let chainwork = add_u256(&prev_chainwork, &work_from_bits(block.header.bits));

    let spent_addrs = spent_addrs_from_utxos(|txid, vout| db.get_utxo(txid, vout), &block)?;

    let batch = build_block_batch(
        height,
        &block,
        block_hash,
        block_size,
        chainwork,
        None,
        None,
        Some(&spent_addrs),
    )?;

    db.put_batch(&[batch])?;

    if fetched.height.is_multiple_of(100) {
        let cl = if fetched.chainlocked { " [CL]" } else { "" };
        println!(
            "  height={} hash={} txs={}{cl}",
            fetched.height,
            &fetched.hash[..16.min(fetched.hash.len())],
            block.transactions.len(),
        );
    }

    Ok(())
}
