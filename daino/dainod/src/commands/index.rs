//! The `index` subcommand -- read block files and index them into LMDB.

use anyhow::{Context, Result};
use std::path::Path;

use daino_core::{BlockFileReader, Network};
use daino_state::db::{BlockRecord, DainoDB, TxRecord};
use librustdash::hash::sha256d;

/// Index block files from a Dash Core data directory into the database.
///
/// Reads blk00000.dat, blk00001.dat, etc. sequentially and stores
/// block headers and transaction metadata in LMDB.
///
/// Note: Block files contain blocks in the order they were received by
/// the node, NOT in chain order. Blocks within a single file are roughly
/// ordered but may include orphan/stale blocks. For now we assign heights
/// sequentially (which is correct for blk00000.dat which starts at genesis)
/// but a proper chain-following implementation will be needed later.
pub fn index_blocks(
    datadir: &Path,
    dbdir: &Path,
    network: Network,
    max_blocks: usize,
) -> Result<()> {
    let blocks_dir = datadir.join("blocks");
    if !blocks_dir.exists() {
        anyhow::bail!(
            "Blocks directory not found: {:?}\n\
             Expected Dash Core data directory with blocks/ subdirectory",
            blocks_dir
        );
    }

    let db = DainoDB::open(dbdir)?;

    // Resume from where we left off
    let start_height = match db.tip_height()? {
        Some(h) => {
            println!("Resuming indexing from height {}", h + 1);
            h + 1
        }
        None => {
            println!("Starting fresh index");
            0
        }
    };

    let limit = if max_blocks == 0 {
        usize::MAX
    } else {
        max_blocks
    };

    let mut current_height = start_height;
    let mut total_txs: u64 = 0;
    let mut file_num = 0u32;

    // Calculate which file and position to start from.
    // For simplicity, we re-scan from file 0 and skip already-indexed blocks.
    // A production indexer would store file offsets for resumption.
    let mut blocks_skipped: u32 = 0;

    loop {
        let block_file = blocks_dir.join(format!("blk{:05}.dat", file_num));
        if !block_file.exists() {
            println!("No more block files (stopped at blk{:05}.dat)", file_num);
            break;
        }

        println!("Reading {:?}", block_file);
        let mut reader = BlockFileReader::new(&block_file, network)
            .with_context(|| format!("Failed to open {:?}", block_file))?;

        loop {
            let block = match reader.read_next_block() {
                Ok(Some(b)) => b,
                Ok(None) => break, // End of file
                Err(e) => {
                    eprintln!(
                        "  Error reading block at offset {}: {}",
                        reader.last_block_start(),
                        e
                    );
                    break;
                }
            };

            // Skip blocks we've already indexed
            if blocks_skipped < start_height {
                blocks_skipped += 1;
                continue;
            }

            let indexed = (current_height - start_height) as usize;
            if indexed >= limit {
                println!("Reached block limit ({})", limit);
                print_summary(&db)?;
                return Ok(());
            }

            // Compute block hash from the serialized header.
            // NOTE: Dash uses X11 for block hashes, not SHA-256d. Since we
            // don't have an X11 implementation, we use SHA-256d as a
            // placeholder identifier. For a production indexer, block hashes
            // should come from dashd's RPC or the LevelDB block index.
            let header_bytes = block.header.serialize()?;
            let block_hash = sha256d(&header_bytes);

            let block_size = block.serialize()?.len() as u32;

            let block_record = BlockRecord {
                height: current_height,
                hash: block_hash,
                prev_hash: block.header.prev_blockhash,
                merkle_root: block.header.merkle_root,
                time: block.header.time,
                bits: block.header.bits,
                nonce: block.header.nonce,
                tx_count: block.transactions.len() as u32,
                size: block_size,
            };

            // Build transaction records
            let mut tx_records = Vec::with_capacity(block.transactions.len());
            for (tx_idx, tx) in block.transactions.iter().enumerate() {
                let txid = tx.txid()?;
                let value_out: i64 = tx.outputs.iter().map(|o| o.value).sum();

                tx_records.push(TxRecord {
                    txid,
                    block_height: current_height,
                    tx_index: tx_idx as u32,
                    version: tx.version,
                    tx_type: tx.tx_type as u16,
                    lock_time: tx.lock_time,
                    value_out,
                    input_count: tx.inputs.len() as u32,
                    output_count: tx.outputs.len() as u32,
                });
            }

            total_txs += tx_records.len() as u64;
            db.put_block(&block_record, &tx_records)?;

            // Progress reporting
            if current_height % 1000 == 0 {
                println!(
                    "  height={} txs={} (file blk{:05}.dat)",
                    current_height, total_txs, file_num,
                );
            }

            current_height += 1;
        }

        file_num += 1;
    }

    print_summary(&db)?;
    Ok(())
}

fn print_summary(db: &DainoDB) -> Result<()> {
    println!();
    println!("Indexing complete: {}", db.status_summary()?);
    Ok(())
}
