//! Simple block file reader for Dash blk*.dat files
//!
//! Reads blocks from Dash Core's block files (blk00000.dat, etc.)

use anyhow::{Context, Result};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use librustdash::Block;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Network magic bytes for identifying blocks
///
/// These magic bytes are stored in big-endian (network byte order) in the block files.
/// They correspond to the pchMessageStart values in Dash Core's chainparams.cpp.
#[derive(Debug, Clone, Copy)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
}

impl Network {
    /// Get the magic bytes for this network
    ///
    /// Note: These are the big-endian values as they appear in the file.
    /// - Mainnet: 0xBF0C6BBD (on disk: bf 0c 6b bd)
    /// - Testnet: 0xFFCAE2CE (on disk: ff ca e2 ce)
    /// - Regtest: 0xFCB7B3DD (on disk: fc b7 b3 dd)
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
    last_block_start: u64,
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
            last_block_start: 0,
        })
    }

    /// Read the next block from the file
    pub fn read_next_block(&mut self) -> Result<Option<Block>> {
        // Record the start position of this block
        self.last_block_start = self.position;

        // Try to read magic bytes (stored in big-endian/network byte order)
        let magic = match self.reader.read_u32::<BigEndian>() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None); // End of file
            }
            Err(e) => return Err(e.into()),
        };

        if magic != self.magic_bytes {
            anyhow::bail!(
                "Invalid magic bytes at position {}: expected 0x{:08X}, found 0x{:08X}",
                self.last_block_start,
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
            format!(
                "Failed to deserialize block at position {}",
                self.last_block_start
            )
        })?;

        Ok(Some(block))
    }

    /// Get current position in the file (after last read)
    #[allow(dead_code)]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the start position of the last block read
    pub fn last_block_start(&self) -> u64 {
        self.last_block_start
    }
}
