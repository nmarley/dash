//! The `follow` subcommand -- connect to a running dashd and continuously index new blocks.
//!
//! This command:
//! 1. Connects to dashd via JSON-RPC
//! 2. Compares our indexed tip with dashd's tip
//! 3. Catches up by fetching blocks via RPC
//! 4. Enters a poll loop to index new blocks as they arrive
//!
//! Each block fetched via RPC as raw hex is deserialized using librustdash,
//! then indexed through the same `build_block_batch` path as file indexing.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use daino_core::{add_u256, work_from_bits};
use daino_fetch::rpc::DashdRpc;
use daino_fetch::{FetchedBlock, catch_up, poll_loop};
use daino_state::db::DainoDB;
use daino_state::{build_block_batch, spent_addrs_from_utxos};
use librustdash::Block;

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

    // Verify connectivity
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

    // Determine our starting point
    let our_tip = db.tip_height()?.unwrap_or(0) as u64;
    let dashd_tip = info.blocks;

    println!("Our tip: {}, dashd tip: {}", our_tip, dashd_tip);

    // Catch up if needed
    if our_tip < dashd_tip {
        let start = if our_tip == 0 { 0 } else { our_tip + 1 };
        catch_up(&rpc, start, dashd_tip, |fetched| {
            index_fetched_block(&db, &fetched)
        })
        .await?;
    }

    println!();
    println!(
        "Caught up to tip {}. Entering follow mode (poll every {}s)...",
        dashd_tip, poll_interval
    );
    println!("Press Ctrl-C to stop.");

    // Enter follow mode
    let current = db.tip_height()?.unwrap_or(0) as u64;
    poll_loop(
        &rpc,
        current,
        Duration::from_secs(poll_interval),
        |fetched| index_fetched_block(&db, &fetched),
    )
    .await
}

/// Deserialize a raw block and index it into the database via the shared apply path.
fn index_fetched_block(db: &DainoDB, fetched: &FetchedBlock) -> Result<()> {
    let block = Block::deserialize(&fetched.raw)
        .with_context(|| format!("Failed to deserialize block at height {}", fetched.height))?;

    let block_hash = block.header.block_hash()?;
    let block_size = fetched.raw.len() as u32;

    let prev_chainwork = if fetched.height > 0 {
        db.get_block_by_height(fetched.height as u32 - 1)?
            .map(|b| b.chainwork)
            .unwrap_or([0u8; 32])
    } else {
        [0u8; 32]
    };
    let chainwork = add_u256(&prev_chainwork, &work_from_bits(block.header.bits));

    // No undo files on the RPC path; resolve spent output addresses from UTXOs.
    let spent_addrs = spent_addrs_from_utxos(|txid, vout| db.get_utxo(txid, vout), &block)?;

    let batch = build_block_batch(
        fetched.height as u32,
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
