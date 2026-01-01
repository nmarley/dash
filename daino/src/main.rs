mod block_reader;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use block_reader::{BlockFileReader, Network};

#[derive(Parser)]
#[command(name = "daino")]
#[command(about = "Dash blockchain block file reader", long_about = None)]
struct Cli {
    /// Path to the block file (e.g., blk00000.dat)
    #[arg(value_name = "BLOCK_FILE")]
    block_file: PathBuf,

    /// Network type (mainnet, testnet, regtest)
    #[arg(short, long, default_value = "mainnet")]
    network: String,

    /// Number of blocks to read (default: 1)
    #[arg(short, long, default_value = "1")]
    count: usize,
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

    println!("Reading blocks from: {:?}", cli.block_file);
    println!("Network: {:?}", network);
    println!("Magic bytes: 0x{:08X}", network.magic_bytes());
    println!();

    // Create block reader
    let mut reader = BlockFileReader::new(&cli.block_file, network)?;

    // Read blocks
    for i in 0..cli.count {
        match reader.read_next_block()? {
            Some(block) => {
                println!("=== Block {} ===", i);
                println!("  Position in file: {} bytes", reader.position());
                println!("  Version: {}", block.header.version);
                println!(
                    "  Previous block: {}",
                    hex::encode(block.header.prev_blockhash)
                );
                println!("  Merkle root: {}", hex::encode(block.header.merkle_root));
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
