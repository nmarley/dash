//! Shared block-apply path: turn a deserialized block into a `BlockBatch`.
//!
//! Both `dainod index` (blk/rev files) and `dainod follow` (RPC) build
//! index writes through this module so the LMDB contents stay identical
//! for the same chain data.

use std::collections::HashMap;

use anyhow::{Context, Result};
use daino_core::CBlockUndo;
use librustdash::Block;
use librustdash::script::analyze_script;

use crate::db::{
    AddrTxRef, BlockBatch, BlockRecord, SpentByEntry, SpentOutpoint, TxRecord, UtxoEntry,
};

/// Build a complete `BlockBatch` from a deserialized block.
///
/// * `txids` -- optional precomputed txids (same order as
///   `block.transactions`). When `None`, each txid is computed here.
/// * `undo` -- optional undo data for input-side address indexing (file
///   indexer path). When absent, `spent_addrs` is used instead.
/// * `spent_addrs` -- optional map of spent outpoint `(txid, vout)` to
///   the 20-byte address hash of the spent output. Used by RPC follow
///   when undo files are not available (looked up from the live UTXO set).
pub fn build_block_batch(
    height: u32,
    block: &Block,
    block_hash: [u8; 32],
    block_size: u32,
    chainwork: [u8; 32],
    txids: Option<&[[u8; 32]]>,
    undo: Option<&CBlockUndo>,
    spent_addrs: Option<&HashMap<([u8; 32], u32), [u8; 20]>>,
) -> Result<BlockBatch> {
    if let Some(ids) = txids
        && ids.len() != block.transactions.len()
    {
        anyhow::bail!(
            "precomputed txid count {} != transaction count {}",
            ids.len(),
            block.transactions.len()
        );
    }

    let block_record = BlockRecord {
        height,
        hash: block_hash,
        prev_hash: block.header.prev_blockhash,
        merkle_root: block.header.merkle_root,
        version: block.header.version,
        time: block.header.time,
        bits: block.header.bits,
        nonce: block.header.nonce,
        tx_count: block.transactions.len() as u32,
        size: block_size,
        chainwork,
    };

    let mut tx_records = Vec::with_capacity(block.transactions.len());
    let mut addr_refs: Vec<([u8; 20], AddrTxRef)> = Vec::new();
    let mut new_utxos: Vec<UtxoEntry> = Vec::new();
    let mut spent: Vec<SpentOutpoint> = Vec::new();
    let mut raw_txs: Vec<([u8; 32], Vec<u8>)> = Vec::with_capacity(block.transactions.len());
    let mut spent_by: Vec<SpentByEntry> = Vec::new();

    for (tx_idx, tx) in block.transactions.iter().enumerate() {
        let txid = if let Some(ids) = txids {
            ids[tx_idx]
        } else {
            tx.txid().with_context(|| {
                format!("Failed to compute txid at height {height} idx {tx_idx}")
            })?
        };

        let value_out: i64 = tx.outputs.iter().map(|o| o.value).sum();

        if let Ok(raw) = tx.serialize() {
            raw_txs.push((txid, raw));
        }

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
            // undo.vtxundo is indexed by (tx_index - 1); coinbase excluded.
            let tx_undo = undo.as_ref().and_then(|u| u.vtxundo.get(tx_idx - 1));

            for (vin_idx, input) in tx.inputs.iter().enumerate() {
                let prev_hash = input.previous_output.hash;
                let prev_n = input.previous_output.n;

                spent.push(SpentOutpoint {
                    txid: prev_hash,
                    vout: prev_n,
                });

                spent_by.push(SpentByEntry {
                    spent_txid: prev_hash,
                    spent_vout: prev_n,
                    spending_txid: txid,
                    spending_vin: vin_idx as u32,
                    spending_height: height,
                });

                // Input-side address indexing: prefer undo coin script,
                // else fallback map (UTXO lookup from follow path).
                let input_addr = if let Some(tu) = tx_undo
                    && let Some(coin) = tu.vprevout.get(vin_idx)
                {
                    analyze_script(&coin.txout.script_pubkey).address_hash
                } else {
                    spent_addrs.and_then(|m| m.get(&(prev_hash, prev_n)).copied())
                };

                if let Some(ah) = input_addr {
                    addr_refs.push((
                        ah,
                        AddrTxRef {
                            block_height: height,
                            txid,
                        },
                    ));
                }
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

    Ok(BlockBatch {
        block: block_record,
        txs: tx_records,
        addr_refs,
        new_utxos,
        spent,
        raw_txs,
        spent_by,
    })
}

/// Collect address hashes for outpoints about to be spent, from the live
/// UTXO set. Used by the RPC follow path when undo data is unavailable.
pub fn spent_addrs_from_utxos(
    lookup: impl Fn(&[u8; 32], u32) -> Result<Option<UtxoEntry>>,
    block: &Block,
) -> Result<HashMap<([u8; 32], u32), [u8; 20]>> {
    let mut map = HashMap::new();
    for tx in &block.transactions {
        if tx.is_coinbase() {
            continue;
        }
        for input in &tx.inputs {
            let hash = input.previous_output.hash;
            let n = input.previous_output.n;
            if map.contains_key(&(hash, n)) {
                continue;
            }
            if let Some(utxo) = lookup(&hash, n)?
                && let Some(ah) = utxo.addr_hash
            {
                map.insert((hash, n), ah);
            }
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use librustdash::tx_type::DashTxType;
    use librustdash::{OutPoint, Transaction, TxIn, TxOut};

    fn p2pkh_script(addr: [u8; 20]) -> Vec<u8> {
        let mut s = vec![0x76, 0xa9, 0x14];
        s.extend_from_slice(&addr);
        s.extend_from_slice(&[0x88, 0xac]);
        s
    }

    fn coinbase_tx(value: i64, addr: [u8; 20]) -> Transaction {
        Transaction {
            version: 1,
            tx_type: DashTxType::Normal,
            inputs: vec![TxIn {
                previous_output: OutPoint {
                    hash: [0u8; 32],
                    n: u32::MAX,
                },
                script_sig: vec![0x01, 0x00],
                sequence: u32::MAX,
            }],
            outputs: vec![TxOut {
                value,
                script_pubkey: p2pkh_script(addr),
            }],
            lock_time: 0,
            extra_payload: None,
        }
    }

    fn spend_tx(prev_txid: [u8; 32], prev_vout: u32, value: i64, addr: [u8; 20]) -> Transaction {
        Transaction {
            version: 1,
            tx_type: DashTxType::Normal,
            inputs: vec![TxIn {
                previous_output: OutPoint {
                    hash: prev_txid,
                    n: prev_vout,
                },
                script_sig: vec![],
                sequence: u32::MAX,
            }],
            outputs: vec![TxOut {
                value,
                script_pubkey: p2pkh_script(addr),
            }],
            lock_time: 0,
            extra_payload: None,
        }
    }

    fn bare_block(txs: Vec<Transaction>) -> Block {
        Block {
            header: librustdash::BlockHeader {
                version: 2,
                prev_blockhash: [0u8; 32],
                merkle_root: [0u8; 32],
                time: 1_390_095_618,
                bits: 0x1e0ffff0,
                nonce: 0,
            },
            transactions: txs,
        }
    }

    #[test]
    fn test_apply_coinbase_only() {
        let addr = [0x11u8; 20];
        let cb = coinbase_tx(50_0000_0000, addr);
        let cb_txid = cb.txid().unwrap();
        let block = bare_block(vec![cb]);
        let hash = [0xAAu8; 32];

        let batch = build_block_batch(0, &block, hash, 200, [0u8; 32], None, None, None).unwrap();

        assert_eq!(batch.block.height, 0);
        assert_eq!(batch.block.hash, hash);
        assert_eq!(batch.block.tx_count, 1);
        assert_eq!(batch.txs.len(), 1);
        assert_eq!(batch.txs[0].txid, cb_txid);
        assert_eq!(batch.txs[0].value_out, 50_0000_0000);
        assert_eq!(batch.raw_txs.len(), 1);
        assert_eq!(batch.raw_txs[0].0, cb_txid);
        assert!(batch.spent.is_empty());
        assert!(batch.spent_by.is_empty());
        assert_eq!(batch.new_utxos.len(), 1);
        assert_eq!(batch.new_utxos[0].addr_hash, Some(addr));
        assert_eq!(batch.addr_refs.len(), 1);
        assert_eq!(batch.addr_refs[0].0, addr);
    }

    #[test]
    fn test_apply_simple_spend_with_fallback_addrs() {
        let miner = [0x11u8; 20];
        let receiver = [0x22u8; 20];

        let cb = coinbase_tx(50_0000_0000, miner);
        let cb_txid = cb.txid().unwrap();
        let block0 = bare_block(vec![cb]);
        let batch0 =
            build_block_batch(0, &block0, [0x01; 32], 100, [0u8; 32], None, None, None).unwrap();
        assert_eq!(batch0.new_utxos.len(), 1);

        let spend = spend_tx(cb_txid, 0, 49_0000_0000, receiver);
        let spend_txid = spend.txid().unwrap();
        let block1 = bare_block(vec![coinbase_tx(25_0000_0000, miner), spend]);

        let mut spent_addrs = HashMap::new();
        spent_addrs.insert((cb_txid, 0u32), miner);

        let batch1 = build_block_batch(
            1,
            &block1,
            [0x02; 32],
            300,
            [1u8; 32],
            None,
            None,
            Some(&spent_addrs),
        )
        .unwrap();

        assert_eq!(batch1.txs.len(), 2);
        assert_eq!(batch1.spent.len(), 1);
        assert_eq!(batch1.spent[0].txid, cb_txid);
        assert_eq!(batch1.spent[0].vout, 0);
        assert_eq!(batch1.spent_by.len(), 1);
        assert_eq!(batch1.spent_by[0].spending_txid, spend_txid);
        assert_eq!(batch1.spent_by[0].spent_txid, cb_txid);
        assert_eq!(batch1.raw_txs.len(), 2);

        // Output-side refs for both txs + input-side ref for miner on spend
        let miner_refs: Vec<_> = batch1
            .addr_refs
            .iter()
            .filter(|(a, r)| *a == miner && r.txid == spend_txid)
            .collect();
        assert_eq!(miner_refs.len(), 1);

        let recv_refs: Vec<_> = batch1
            .addr_refs
            .iter()
            .filter(|(a, _)| *a == receiver)
            .collect();
        assert_eq!(recv_refs.len(), 1);
    }

    #[test]
    fn test_precomputed_txids() {
        let addr = [0x33u8; 20];
        let cb = coinbase_tx(1, addr);
        let real_txid = cb.txid().unwrap();
        let block = bare_block(vec![cb]);
        let fake = [[0xFFu8; 32]];

        let batch =
            build_block_batch(0, &block, [0; 32], 10, [0; 32], Some(&fake), None, None).unwrap();
        assert_eq!(batch.txs[0].txid, fake[0]);
        assert_ne!(batch.txs[0].txid, real_txid);
    }
}
