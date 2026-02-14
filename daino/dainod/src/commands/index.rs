//! The `index` subcommand -- read block files and index them into LMDB.
//!
//! Uses a two-pass architecture for correct chain ordering:
//! - Pass 1: Scan all block headers to build a block_hash -> file location map
//! - Chain walk: Follow prev_hash links from genesis to derive correct heights
//! - Pass 2: Re-read full blocks in chain order and index them
//!
//! Pass 2 uses a read-ahead pipeline: a dedicated reader thread handles
//! disk I/O, deserialization, and hashing, sending parsed blocks through a
//! bounded channel to the main thread which does UTXO/address extraction
//! and LMDB writes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result};

use daino_core::{BlockFileReader, Network, ScannedHeader};
use daino_state::db::{
    AddrTxRef, BlockBatch, BlockRecord, DainoDB, SpentOutpoint, TxRecord, UtxoEntry,
};
use librustdash::script::analyze_script;
use librustdash::Block;

/// Location of a block within the blk file set.
#[derive(Debug, Clone)]
struct BlockLocation {
    file_num: u32,
    file_offset: u64,
    block_size: u32,
}

/// A parsed block ready for indexing, produced by the reader thread.
struct ReadBlock {
    height: u32,
    block: Block,
    block_hash: [u8; 32],
    /// Precomputed txids (one per transaction, same order as block.transactions)
    txids: Vec<[u8; 32]>,
    /// Block size including magic + size prefix
    block_size: u32,
}

/// Read-ahead channel capacity (number of parsed blocks to buffer).
const READ_AHEAD: usize = 500;

/// Index block files into the database.
///
/// Two-pass architecture:
/// 1. Scan all headers to discover blocks and their prev_hash links
/// 2. Walk the chain from genesis to assign correct heights
/// 3. Re-read full blocks in chain order and index into LMDB
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

    // ── Pass 1: Scan all block headers ──────────────────────────────

    let t_start = Instant::now();
    println!("Pass 1: Scanning block headers...");

    let mut all_headers: Vec<(ScannedHeader, u32)> = Vec::new(); // (header, file_num)
    let mut file_num = 0u32;

    loop {
        let block_file = datadir.join(format!("blk{:05}.dat", file_num));
        if !block_file.exists() {
            break;
        }

        let mut reader = BlockFileReader::new(&block_file, network)
            .with_context(|| format!("Failed to open {:?}", block_file))?;

        match reader.scan_headers() {
            Ok(headers) => {
                let count = headers.len();
                for h in headers {
                    all_headers.push((h, file_num));
                }
                println!("  blk{:05}.dat: {} blocks", file_num, count);
            }
            Err(e) => {
                eprintln!("  blk{:05}.dat: error scanning headers: {}", file_num, e);
            }
        }

        file_num += 1;
    }

    let t_scan = t_start.elapsed();
    println!(
        "Scanned {} blocks from {} files in {:.1}s",
        all_headers.len(),
        file_num,
        t_scan.as_secs_f64(),
    );

    if all_headers.is_empty() {
        println!("No blocks found.");
        return Ok(());
    }

    // ── Chain walk: derive height assignments ────────────────────────

    println!("Building chain order...");

    // Build lookup: block_hash -> (index into all_headers)
    let mut hash_to_idx: HashMap<[u8; 32], usize> = HashMap::with_capacity(all_headers.len());
    for (i, (header, _file_num)) in all_headers.iter().enumerate() {
        hash_to_idx.insert(header.block_hash, i);
    }

    // Find genesis: the block whose prev_hash is all zeros
    let genesis_hash = {
        let zero_prev = [0u8; 32];
        let mut genesis = None;
        for (header, _) in &all_headers {
            if header.prev_hash == zero_prev {
                genesis = Some(header.block_hash);
                break;
            }
        }
        genesis.ok_or_else(|| {
            anyhow::anyhow!("No genesis block found (no block with prev_hash = 0)")
        })?
    };

    // Build forward links: prev_hash -> Vec<child_hash>
    // (Vec because there can be forks)
    let mut children: HashMap<[u8; 32], Vec<[u8; 32]>> = HashMap::with_capacity(all_headers.len());
    for (header, _) in &all_headers {
        children
            .entry(header.prev_hash)
            .or_default()
            .push(header.block_hash);
    }

    // Walk the longest chain from genesis.
    // At each fork, pick the branch with the most descendants.
    let chain_hashes = {
        let mut chain = Vec::with_capacity(all_headers.len());
        let mut current = genesis_hash;

        loop {
            chain.push(current);

            match children.get(&current) {
                None => break,
                Some(kids) if kids.is_empty() => break,
                Some(kids) if kids.len() == 1 => {
                    current = kids[0];
                }
                Some(kids) => {
                    let mut best = kids[0];
                    let mut best_len = count_chain_length(&children, kids[0]);
                    for &kid in &kids[1..] {
                        let len = count_chain_length(&children, kid);
                        if len > best_len {
                            best = kid;
                            best_len = len;
                        }
                    }
                    current = best;
                }
            }
        }

        chain
    };

    println!(
        "Chain: {} blocks (genesis to height {})",
        chain_hashes.len(),
        chain_hashes.len() - 1,
    );

    let orphan_count = all_headers.len() - chain_hashes.len();
    if orphan_count > 0 {
        println!("Skipping {} orphan/stale blocks", orphan_count);
    }

    // Build the location map for each block hash in chain order
    let chain_locations: Vec<BlockLocation> = chain_hashes
        .iter()
        .map(|hash| {
            let idx = hash_to_idx[hash];
            let (header, fnum) = &all_headers[idx];
            BlockLocation {
                file_num: *fnum,
                file_offset: header.file_offset,
                block_size: header.block_size,
            }
        })
        .collect();

    // ── Pass 2: Read full blocks in chain order and index ────────────

    let end_height = if max_blocks == 0 {
        chain_hashes.len() as u32
    } else {
        std::cmp::min(
            chain_hashes.len() as u32,
            start_height.saturating_add(max_blocks as u32),
        )
    };

    if start_height as usize >= chain_hashes.len() {
        println!("Already fully indexed up to height {}.", start_height - 1);
        print_summary(&db, t_start)?;
        return Ok(());
    }

    println!(
        "Pass 2: Indexing blocks {} to {} (read-ahead={}) ...",
        start_height,
        end_height - 1,
        READ_AHEAD,
    );

    // Spawn reader thread with bounded channel
    let (tx, rx) = mpsc::sync_channel::<ReadBlock>(READ_AHEAD);

    let reader_datadir = PathBuf::from(datadir);
    let reader_locations: Vec<BlockLocation> =
        chain_locations[start_height as usize..end_height as usize].to_vec();
    let reader_start = start_height;

    let reader_handle = std::thread::spawn(move || -> Result<()> {
        let mut readers: HashMap<u32, BlockFileReader> = HashMap::new();

        for (i, loc) in reader_locations.iter().enumerate() {
            let height = reader_start + i as u32;

            // Get or open the reader for this file
            let reader = if let Some(r) = readers.get_mut(&loc.file_num) {
                r
            } else {
                let path = reader_datadir.join(format!("blk{:05}.dat", loc.file_num));
                let r = BlockFileReader::new(&path, network)
                    .with_context(|| format!("Failed to open {:?}", path))?;
                readers.entry(loc.file_num).or_insert(r)
            };

            let block = reader
                .read_block_at(loc.file_offset)
                .with_context(|| format!("Failed to read block at height {}", height))?;

            // Compute hashes on the reader thread (X11 + SHA-256d)
            let block_hash = block.header.block_hash()?;
            let mut txids = Vec::with_capacity(block.transactions.len());
            for tx in &block.transactions {
                txids.push(tx.txid()?);
            }

            let block_size = loc.block_size + 8; // +8 for magic + size prefix

            let read_block = ReadBlock {
                height,
                block,
                block_hash,
                txids,
                block_size,
            };

            // Send to writer; if receiver is dropped, stop
            if tx.send(read_block).is_err() {
                break;
            }
        }

        Ok(())
    });

    // ── Writer: receive parsed blocks and index into LMDB ───────────

    let mut batch: Vec<BlockBatch> = Vec::with_capacity(batch_size);
    let mut total_txs: u64 = 0;
    let t_pass2 = Instant::now();

    for read_block in rx {
        let ReadBlock {
            height,
            block,
            block_hash,
            txids,
            block_size,
        } = read_block;

        let block_record = BlockRecord {
            height,
            hash: block_hash,
            prev_hash: block.header.prev_blockhash,
            merkle_root: block.header.merkle_root,
            time: block.header.time,
            bits: block.header.bits,
            nonce: block.header.nonce,
            tx_count: block.transactions.len() as u32,
            size: block_size,
        };

        let mut tx_records = Vec::with_capacity(block.transactions.len());
        let mut addr_refs: Vec<([u8; 20], AddrTxRef)> = Vec::new();
        let mut new_utxos: Vec<UtxoEntry> = Vec::new();
        let mut spent: Vec<SpentOutpoint> = Vec::new();

        for (tx_idx, tx) in block.transactions.iter().enumerate() {
            let txid = txids[tx_idx];
            let value_out: i64 = tx.outputs.iter().map(|o| o.value).sum();

            tx_records.push(TxRecord {
                txid,
                block_height: height,
                tx_index: tx_idx as u32,
                version: tx.version,
                tx_type: tx.tx_type as u16,
                lock_time: tx.lock_time,
                value_out,
                input_count: tx.inputs.len() as u32,
                output_count: tx.outputs.len() as u32,
            });

            if !tx.is_coinbase() {
                for input in &tx.inputs {
                    spent.push(SpentOutpoint {
                        txid: input.previous_output.hash,
                        vout: input.previous_output.n,
                    });
                }
            }

            for (vout, output) in tx.outputs.iter().enumerate() {
                let info = analyze_script(&output.script_pubkey);
                let addr_hash = info.address_hash;

                if let Some(ah) = addr_hash {
                    addr_refs.push((
                        ah,
                        AddrTxRef {
                            block_height: height,
                            txid,
                        },
                    ));
                }

                new_utxos.push(UtxoEntry {
                    txid,
                    vout: vout as u32,
                    value: output.value,
                    block_height: height,
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

        if batch.len() >= batch_size {
            db.put_batch(&batch)?;
            batch.clear();
        }

        // Progress reporting every 1000 blocks
        if height % 1000 == 0 && height > start_height {
            let elapsed = t_pass2.elapsed().as_secs_f64();
            let total_blocks = (height - start_height) as f64;
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
                "  height={} txs={} | {:.0} blk/s {:.0} tx/s | {:.1}s elapsed",
                height, total_txs, blk_per_sec, tx_per_sec, elapsed,
            );
        }
    }

    // Flush remaining batch
    if !batch.is_empty() {
        db.put_batch(&batch)?;
    }

    // Wait for reader thread and propagate any errors
    match reader_handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e.context("Reader thread failed")),
        Err(_) => anyhow::bail!("Reader thread panicked"),
    }

    print_summary(&db, t_start)?;
    Ok(())
}

/// Count how many blocks follow `start` in the chain (for fork resolution).
fn count_chain_length(children: &HashMap<[u8; 32], Vec<[u8; 32]>>, start: [u8; 32]) -> usize {
    let mut len = 1;
    let mut current = start;
    loop {
        match children.get(&current) {
            Some(kids) if !kids.is_empty() => {
                current = kids[0];
                len += 1;
            }
            _ => break,
        }
    }
    len
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
