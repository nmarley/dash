//! The `index` subcommand -- read block files and index them into LMDB.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

use daino_core::{BlockFileReader, Network};
use daino_state::db::{
    AddrTxRef, BlockBatch, BlockRecord, DainoDB, SpentOutpoint, TxRecord, UtxoEntry,
};
use librustdash::script::analyze_script;

/// Index block files into the database.
///
/// Reads blk00000.dat, blk00001.dat, etc. sequentially and stores
/// block headers, transactions, address index, and UTXO set in LMDB.
///
/// Uses batched writes (configurable via `batch_size`) to minimize
/// fsync overhead -- the main bottleneck during bulk indexing.
pub fn index_blocks(
    datadir: &Path,
    dbdir: &Path,
    network: Network,
    max_blocks: usize,
    batch_size: usize,
) -> Result<()> {
    if !datadir.exists() {
        anyhow::bail!(
            "Directory not found: {:?}\n\
             Expected directory containing blk*.dat files",
            datadir
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

    // Batching state
    let mut batch: Vec<BlockBatch> = Vec::with_capacity(batch_size);

    // Timing
    let t_start = Instant::now();
    let mut t_last_report = t_start;

    // For resumption: skip already-indexed blocks
    let mut blocks_skipped: u32 = 0;

    loop {
        let block_file = datadir.join(format!("blk{:05}.dat", file_num));
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
                // Flush remaining batch
                if !batch.is_empty() {
                    db.put_batch(&batch)?;
                    batch.clear();
                }
                println!("Reached block limit ({})", limit);
                print_summary(&db, t_start)?;
                return Ok(());
            }

            // Compute block hash using X11
            let block_hash = block.header.block_hash()?;
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
                    block_height: current_height,
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
                                block_height: current_height,
                                txid,
                            },
                        ));
                    }

                    new_utxos.push(UtxoEntry {
                        txid,
                        vout: vout as u32,
                        value: output.value,
                        block_height: current_height,
                        addr_hash,
                    });
                }
            }

            total_txs += tx_records.len() as u64;

            batch.push(BlockBatch {
                block: block_record,
                txs: tx_records,
                addr_refs,
                new_utxos,
                spent,
            });

            // Flush batch when full
            if batch.len() >= batch_size {
                db.put_batch(&batch)?;
                batch.clear();
            }

            // Progress reporting every 1000 blocks
            if current_height % 1000 == 0 && current_height > start_height {
                let now = Instant::now();
                let elapsed = now.duration_since(t_start).as_secs_f64();
                let _interval = now.duration_since(t_last_report).as_secs_f64();
                let total_blocks = (current_height - start_height) as f64;
                let blk_per_sec = if elapsed > 0.0 {
                    total_blocks / elapsed
                } else {
                    0.0
                };
                let tx_per_sec = if elapsed > 0.0 {
                    total_txs as f64 / elapsed
                } else {
                    0.0
                };

                println!(
                    "  height={} txs={} | {:.0} blk/s {:.0} tx/s | {:.1}s elapsed (blk{:05}.dat)",
                    current_height, total_txs, blk_per_sec, tx_per_sec, elapsed, file_num,
                );
                t_last_report = now;
            }

            current_height += 1;
        }

        file_num += 1;
    }

    // Flush remaining batch
    if !batch.is_empty() {
        db.put_batch(&batch)?;
    }

    print_summary(&db, t_start)?;
    Ok(())
}

fn print_summary(db: &DainoDB, t_start: Instant) -> Result<()> {
    let elapsed = t_start.elapsed().as_secs_f64();
    let meta = db.get_meta()?;

    println!();
    if let Some(m) = meta {
        let blk_per_sec = if elapsed > 0.0 {
            m.block_count as f64 / elapsed
        } else {
            0.0
        };
        let tx_per_sec = if elapsed > 0.0 {
            m.tx_count as f64 / elapsed
        } else {
            0.0
        };

        let elapsed_fmt = if elapsed >= 60.0 {
            format!("{}m{:.0}s", (elapsed / 60.0) as u64, elapsed % 60.0)
        } else {
            format!("{:.1}s", elapsed)
        };

        println!(
            "Indexing complete: {} blocks, {} txs in {} ({:.0} blk/s, {:.0} tx/s)",
            m.block_count, m.tx_count, elapsed_fmt, blk_per_sec, tx_per_sec,
        );
        println!(
            "Tip: height={} hash={}",
            m.tip_height,
            librustdash::hash::hash_to_display(&m.tip_hash),
        );
    } else {
        println!("Indexing complete: empty database");
    }

    Ok(())
}
