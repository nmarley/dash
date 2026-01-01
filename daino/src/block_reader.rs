//! Simple block file reader for Dash blk*.dat files
//!
//! Reads blocks from Dash Core's block files (blk00000.dat, etc.)

use anyhow::{Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use librustdash::Block;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Network magic bytes for identifying blocks
#[derive(Debug, Clone, Copy)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
}

impl Network {
    /// Get the magic bytes for this network
    pub fn magic_bytes(&self) -> u32 {
        match self {
            Network::Mainnet => 0xBF0C6BBD,
            Network::Testnet => 0xFFCAE2CE,
            Network::Regtest => 0xFCB7B3DD,
        }
    }
}

/// Simple block file reader
pub struct BlockFileReader {
    reader: BufReader<File>,
    magic_bytes: u32,
    position: u64,
}

impl BlockFileReader {
    /// Create a new block file reader
    pub fn new<P: AsRef<Path>>(path: P, network: Network) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open block file: {:?}", path.as_ref()))?;
        let reader = BufReader::new(file);

        Ok(Self {
            reader,
            magic_bytes: network.magic_bytes(),
            position: 0,
        })
    }

    /// Read the next block from the file
    pub fn read_next_block(&mut self) -> Result<Option<Block>> {
        // Try to read magic bytes
        let magic = match self.reader.read_u32::<LittleEndian>() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None); // End of file
            }
            Err(e) => return Err(e.into()),
        };

        if magic != self.magic_bytes {
            anyhow::bail!(
                "Invalid magic bytes at position {}: expected 0x{:08X}, found 0x{:08X}",
                self.position,
                self.magic_bytes,
                magic
            );
        }

        self.position += 4;

        // Read block size
        let size = self
            .reader
            .read_u32::<LittleEndian>()
            .context("Failed to read block size")?;
        self.position += 4;

        // Read block data
        let mut block_data = vec![0u8; size as usize];
        self.reader
            .read_exact(&mut block_data)
            .context("Failed to read block data")?;
        self.position += size as u64;

        // Deserialize using librustdash
        let block = Block::deserialize(&block_data).with_context(|| {
            format!("Failed to deserialize block at position {}", self.position)
        })?;

        Ok(Some(block))
    }

    /// Get current position in the file
    pub fn position(&self) -> u64 {
        self.position
    }
}
