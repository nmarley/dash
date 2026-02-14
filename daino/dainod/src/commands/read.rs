//! The `read` subcommand -- read and display blocks/undo data from raw files.

use anyhow::Result;
use std::path::Path;

use daino_core::{BlockFileReader, Network, UndoFileReader};

pub fn read_undo_file(file: &Path, network: Network, count: usize) -> Result<()> {
    println!("Reading undo data from: {:?}", file);
    println!("Network: {:?}", network);
    println!("Magic bytes: 0x{:08X}", network.magic_bytes());
    println!();

    let mut reader = UndoFileReader::new(file, network)?;

    for i in 0..count {
        match reader.read_next_undo(None)? {
            Some(block_undo) => {
                println!("=== Undo Block {} ===", i);
                println!("  Position in file: {} bytes", reader.last_undo_start());
                println!("  Transaction undo count: {}", block_undo.vtxundo.len());

                for (tx_idx, tx_undo) in block_undo.vtxundo.iter().enumerate() {
                    println!("  Transaction {} (non-coinbase):", tx_idx + 1);
                    println!("    Spent outputs count: {}", tx_undo.vprevout.len());

                    for (out_idx, coin) in tx_undo.vprevout.iter().enumerate() {
                        println!("      Output {}:", out_idx);
                        println!("        Value: {} satoshis", coin.txout.value);
                        println!(
                            "        ScriptPubKey: {} bytes",
                            coin.txout.script_pubkey.len()
                        );
                        println!(
                            "        ScriptPubKey (hex): {}",
                            hex::encode(&coin.txout.script_pubkey)
                        );
                        println!("        Height: {}", coin.height);
                        println!("        Is coinbase: {}", coin.is_coinbase);
                    }
                }

                println!();
            }
            None => {
                println!("Reached end of file after {} undo blocks", i);
                break;
            }
        }
    }

    Ok(())
}

pub fn read_block_file(
    file: &Path,
    network: Network,
    count: usize,
    show_coinbase_message: bool,
    show_raw_block: bool,
) -> Result<()> {
    println!("Reading blocks from: {:?}", file);
    println!("Network: {:?}", network);
    println!("Magic bytes: 0x{:08X}", network.magic_bytes());
    println!();

    let mut reader = BlockFileReader::new(file, network)?;

    for i in 0..count {
        match reader.read_next_block()? {
            Some(block) => {
                let coinbase_txid = if !block.transactions.is_empty() {
                    block.transactions[0].txid_hex().ok()
                } else {
                    None
                };

                println!("=== Block {} ===", i);
                println!("  Position in file: {} bytes", reader.last_block_start());
                println!("  Version: {}", block.header.version);

                let mut prev_hash_reversed = block.header.prev_blockhash;
                prev_hash_reversed.reverse();
                println!("  Previous block: {}", hex::encode(prev_hash_reversed));

                let mut merkle_root_reversed = block.header.merkle_root;
                merkle_root_reversed.reverse();
                println!("  Merkle root: {}", hex::encode(merkle_root_reversed));
                println!("  Timestamp: {}", block.header.time);
                println!("  Bits: 0x{:08X}", block.header.bits);
                println!("  Nonce: {}", block.header.nonce);
                println!("  Transaction count: {}", block.transactions.len());

                if !block.transactions.is_empty() {
                    let coinbase = &block.transactions[0];
                    println!("  Coinbase tx:");
                    if let Some(ref txid) = coinbase_txid {
                        println!("    Txid: {}", txid);
                    }
                    println!("    Version: {}", coinbase.version);
                    println!("    Type: {:?}", coinbase.tx_type);
                    println!("    Inputs: {}", coinbase.inputs.len());
                    println!("    Outputs: {}", coinbase.outputs.len());
                    if let Some(ref payload) = coinbase.extra_payload {
                        println!("    Extra payload: {} bytes", payload.len());
                    }

                    if show_coinbase_message
                        && !coinbase.inputs.is_empty()
                        && let Some(message) =
                            extract_coinbase_message(&coinbase.inputs[0].script_sig)
                    {
                        println!("  Coinbase message:");
                        println!("    \"{}\"", message);
                    }
                }

                if show_raw_block {
                    let header_bytes = block.header.serialize()?;
                    println!("  Raw block header (80 bytes):");
                    println!("    {}", hex::encode(&header_bytes));
                }

                println!();
            }
            None => {
                println!("Reached end of file after {} blocks", i);
                break;
            }
        }
    }

    Ok(())
}

/// Extract the coinbase message from a coinbase transaction's scriptsig
fn extract_coinbase_message(script_sig: &[u8]) -> Option<String> {
    let mut i = 0;
    let mut best_message: Option<String> = None;
    let mut best_len = 0;

    while i < script_sig.len() {
        let opcode = script_sig[i];
        i += 1;

        let data_len = if opcode <= 0x4b {
            opcode as usize
        } else if opcode == 0x4c && i < script_sig.len() {
            let len = script_sig[i] as usize;
            i += 1;
            len
        } else if opcode == 0x4d && i + 1 < script_sig.len() {
            let len = script_sig[i] as usize | ((script_sig[i + 1] as usize) << 8);
            i += 2;
            len
        } else if opcode == 0x4e && i + 3 < script_sig.len() {
            let len = script_sig[i] as usize
                | ((script_sig[i + 1] as usize) << 8)
                | ((script_sig[i + 2] as usize) << 16)
                | ((script_sig[i + 3] as usize) << 24);
            i += 4;
            len
        } else {
            continue;
        };

        if i + data_len <= script_sig.len() {
            let data = &script_sig[i..i + data_len];
            i += data_len;

            let printable_count = data.iter().filter(|&&b| (0x20..=0x7e).contains(&b)).count();

            if data_len > best_len
                && printable_count * 100 / data_len >= 80
                && data_len >= 10
                && let Ok(msg) = String::from_utf8(data.to_vec())
            {
                best_len = data_len;
                best_message = Some(msg);
            }
        } else {
            break;
        }
    }

    best_message
}
