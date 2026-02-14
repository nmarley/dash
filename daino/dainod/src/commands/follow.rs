//! The `follow` subcommand -- connect to a running dashd and continuously index new blocks.
//!
//! This command:
//! 1. Connects to dashd via JSON-RPC
//! 2. Compares our indexed tip with dashd's tip
//! 3. Catches up by fetching blocks via RPC
//! 4. Enters a poll loop to index new blocks as they arrive
//!
//! Each block fetched via RPC as raw hex is deserialized using librustdash,
//! then indexed the same way as the `index` command does for block files.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use daino_fetch::rpc::DashdRpc;
use daino_fetch::{FetchedBlock, catch_up, poll_loop};
use daino_state::db::{AddrTxRef, BlockRecord, DainoDB, SpentOutpoint, TxRecord, UtxoEntry};
use librustdash::Block;
use librustdash::script::analyze_script;

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

/// Deserialize a raw block and index it into the database.
fn index_fetched_block(db: &DainoDB, fetched: &FetchedBlock) -> Result<()> {
    // Deserialize the raw block bytes using librustdash
    let block = Block::deserialize(&fetched.raw)
        .with_context(|| format!("Failed to deserialize block at height {}", fetched.height))?;

    // Compute block hash using X11
    let block_hash = block.header.block_hash()?;
    let block_size = fetched.raw.len() as u32;

    let block_record = BlockRecord {
        height: fetched.height as u32,
        hash: block_hash,
        prev_hash: block.header.prev_blockhash,
        merkle_root: block.header.merkle_root,
        time: block.header.time,
        bits: block.header.bits,
        nonce: block.header.nonce,
        tx_count: block.transactions.len() as u32,
        size: block_size,
    };

    // Build transaction records, address index, and UTXO updates
    let mut tx_records = Vec::with_capacity(block.transactions.len());
    let mut addr_refs: Vec<([u8; 20], AddrTxRef)> = Vec::new();
    let mut new_utxos: Vec<UtxoEntry> = Vec::new();
    let mut spent: Vec<SpentOutpoint> = Vec::new();

    for (tx_idx, tx) in block.transactions.iter().enumerate() {
        let txid = tx.txid()?;
        let value_out: i64 = tx.outputs.iter().map(|o| o.value).sum();

        tx_records.push(TxRecord {
            txid,
            block_height: fetched.height as u32,
            tx_index: tx_idx as u32,
            version: tx.version,
            tx_type: tx.tx_type as u16,
            lock_time: tx.lock_time,
            value_out,
            input_count: tx.inputs.len() as u32,
            output_count: tx.outputs.len() as u32,
        });

        // Collect spent outpoints from inputs (skip coinbase)
        if !tx.is_coinbase() {
            for input in &tx.inputs {
                spent.push(SpentOutpoint {
                    txid: input.previous_output.hash,
                    vout: input.previous_output.n,
                });
            }
        }

        // Extract address hashes from outputs for address + UTXO indexing
        for (vout, output) in tx.outputs.iter().enumerate() {
            let info = analyze_script(&output.script_pubkey);
            let addr_hash = info.address_hash;

            if let Some(ah) = addr_hash {
                addr_refs.push((
                    ah,
                    AddrTxRef {
                        block_height: fetched.height as u32,
                        txid,
                    },
                ));
            }

            new_utxos.push(UtxoEntry {
                txid,
                vout: vout as u32,
                value: output.value,
                block_height: fetched.height as u32,
                addr_hash,
            });
        }
    }

    db.put_block(&block_record, &tx_records, &addr_refs, &new_utxos, &spent)?;

    // Progress reporting
    if fetched.height.is_multiple_of(100) {
        let cl = if fetched.chainlocked { " [CL]" } else { "" };
        println!(
            "  height={} hash={} txs={}{cl}",
            fetched.height,
            &fetched.hash[..16],
            block.transactions.len(),
        );
    }

    Ok(())
}
