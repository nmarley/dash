mod block_reader;
mod undo;
mod undo_reader;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use block_reader::{BlockFileReader, Network};
use undo_reader::UndoFileReader;

#[derive(Parser)]
#[command(name = "daino")]
#[command(about = "Dash blockchain block and undo file reader", long_about = None)]
struct Cli {
    /// Path to the file (e.g., blk00000.dat or rev00000.dat)
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Network type (mainnet, testnet, regtest)
    #[arg(short, long, default_value = "mainnet")]
    network: String,

    /// Number of blocks/undo records to read (default: 1)
    #[arg(short, long, default_value = "1")]
    count: usize,

    /// Read undo file (rev*.dat) instead of block file
    #[arg(short, long)]
    undo: bool,

    /// Show coinbase message (useful for genesis block) - block files only
    #[arg(long)]
    show_coinbase_message: bool,

    /// Show raw block header bytes (for POW verification) - block files only
    #[arg(long)]
    show_raw_block: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Parse network
    let network = match cli.network.as_str() {
        "mainnet" => Network::Mainnet,
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        _ => anyhow::bail!("Invalid network: {}", cli.network),
    };

    if cli.undo {
        read_undo_file(&cli, network)
    } else {
        read_block_file(&cli, network)
    }
}

fn read_undo_file(cli: &Cli, network: Network) -> Result<()> {
    println!("Reading undo data from: {:?}", cli.file);
    println!("Network: {:?}", network);
    println!("Magic bytes: 0x{:08X}", network.magic_bytes());
    println!();

    // Create undo reader
    let mut reader = UndoFileReader::new(&cli.file, network)?;

    // Read undo blocks
    for i in 0..cli.count {
        // Note: We skip checksum verification since we don't have block data
        match reader.read_next_undo(None)? {
            Some(block_undo) => {
                println!("=== Undo Block {} ===", i);
                println!("  Position in file: {} bytes", reader.last_undo_start());
                println!("  Transaction undo count: {}", block_undo.vtxundo.len());

                // Show details of each transaction's undo data
                for (tx_idx, tx_undo) in block_undo.vtxundo.iter().enumerate() {
                    println!("  Transaction {} (non-coinbase):", tx_idx + 1);
                    println!("    Spent outputs count: {}", tx_undo.vprevout.len());

                    // Show details of each spent output
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

fn read_block_file(cli: &Cli, network: Network) -> Result<()> {
    println!("Reading blocks from: {:?}", cli.file);
    println!("Network: {:?}", network);
    println!("Magic bytes: 0x{:08X}", network.magic_bytes());
    println!();

    // Create block reader
    let mut reader = BlockFileReader::new(&cli.file, network)?;

    // Read blocks
    for i in 0..cli.count {
        match reader.read_next_block()? {
            Some(block) => {
                println!("=== Block {} ===", i);
                println!("  Position in file: {} bytes", reader.last_block_start());
                println!("  Version: {}", block.header.version);

                // Reverse byte order for display (internal -> RPC byte order)
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

                // Show first transaction (coinbase)
                if !block.transactions.is_empty() {
                    let coinbase = &block.transactions[0];
                    println!("  Coinbase tx:");
                    println!("    Version: {}", coinbase.version);
                    println!("    Type: {:?}", coinbase.tx_type);
                    println!("    Inputs: {}", coinbase.inputs.len());
                    println!("    Outputs: {}", coinbase.outputs.len());
                    if let Some(ref payload) = coinbase.extra_payload {
                        println!("    Extra payload: {} bytes", payload.len());
                    }

                    // Show coinbase message if requested
                    if cli.show_coinbase_message && !coinbase.inputs.is_empty()
                        && let Some(message) =
                            extract_coinbase_message(&coinbase.inputs[0].script_sig)
                        {
                            println!("  Coinbase message:");
                            println!("    \"{}\"", message);
                        }
                }

                // Show raw block header if requested
                if cli.show_raw_block {
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
    // The coinbase message is embedded in the scriptSig after the block height push
    // For genesis block and early blocks, the format is typically:
    // [height_script_bytes] [push_opcode] [message_length] [message_bytes] ...
    //
    // Bitcoin script opcodes:
    // 0x01-0x4b: Push next N bytes
    // 0x4c (OP_PUSHDATA1): Next byte = length, then push that many bytes
    // 0x4d (OP_PUSHDATA2): Next 2 bytes = length, then push that many bytes
    // 0x4e (OP_PUSHDATA4): Next 4 bytes = length, then push that many bytes

    let mut i = 0;
    let mut best_message: Option<String> = None;
    let mut best_len = 0;

    while i < script_sig.len() {
        let opcode = script_sig[i];
        i += 1;

        // Determine how many bytes to read based on opcode
        let data_len = if opcode <= 0x4b {
            // Direct push of N bytes
            opcode as usize
        } else if opcode == 0x4c && i < script_sig.len() {
            // OP_PUSHDATA1: next byte is length
            let len = script_sig[i] as usize;
            i += 1;
            len
        } else if opcode == 0x4d && i + 1 < script_sig.len() {
            // OP_PUSHDATA2: next 2 bytes are length (little-endian)
            let len = script_sig[i] as usize | ((script_sig[i + 1] as usize) << 8);
            i += 2;
            len
        } else if opcode == 0x4e && i + 3 < script_sig.len() {
            // OP_PUSHDATA4: next 4 bytes are length (little-endian)
            let len = script_sig[i] as usize
                | ((script_sig[i + 1] as usize) << 8)
                | ((script_sig[i + 2] as usize) << 16)
                | ((script_sig[i + 3] as usize) << 24);
            i += 4;
            len
        } else {
            // Unknown opcode or insufficient data, try to continue
            continue;
        };

        // Extract the data
        if i + data_len <= script_sig.len() {
            let data = &script_sig[i..i + data_len];
            i += data_len;

            // Check if this data is mostly printable ASCII
            let printable_count = data.iter().filter(|&&b| (0x20..=0x7e).contains(&b)).count();

            // If at least 80% is printable and it's reasonably long, consider it a message
            if data_len > best_len && printable_count * 100 / data_len >= 80 && data_len >= 10
                && let Ok(msg) = String::from_utf8(data.to_vec()) {
                    best_len = data_len;
                    best_message = Some(msg);
                }
        } else {
            break;
        }
    }

    best_message
}
