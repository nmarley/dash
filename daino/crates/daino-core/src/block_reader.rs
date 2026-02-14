//! Block file reader for Dash blk*.dat files
//!
//! Reads blocks from Dash Core's block files (blk00000.dat, etc.)
//! Supports both sequential reading and seek-based random access.

use anyhow::{Context, Result};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use librustdash::block::BlockHeader;
use librustdash::Block;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
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

    /// Scan all block headers in the file without deserializing transactions.
    ///
    /// For each block, reads the 80-byte header and skips the rest.
    /// Returns a list of `ScannedHeader` entries with block hash, prev hash,
    /// and the file offset needed to re-read the full block later.
    pub fn scan_headers(&mut self) -> Result<Vec<ScannedHeader>> {
        // Seek to start of file
        self.reader.seek(SeekFrom::Start(0))?;
        self.position = 0;
        self.last_block_start = 0;

        let mut headers = Vec::new();

        loop {
            let offset = self.position;

            // Read magic bytes
            let magic = match self.reader.read_u32::<BigEndian>() {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            };
            self.position += 4;

            if magic != self.magic_bytes {
                anyhow::bail!(
                    "Invalid magic bytes at position {}: expected 0x{:08X}, found 0x{:08X}",
                    offset,
                    self.magic_bytes,
                    magic
                );
            }

            // Read block size
            let block_size = self
                .reader
                .read_u32::<LittleEndian>()
                .context("Failed to read block size")?;
            self.position += 4;

            // Read only the 80-byte header
            if block_size < 80 {
                anyhow::bail!(
                    "Block size {} is too small for a header at position {}",
                    block_size,
                    offset
                );
            }

            let mut header_bytes = [0u8; 80];
            self.reader
                .read_exact(&mut header_bytes)
                .context("Failed to read block header")?;
            self.position += 80;

            let header = BlockHeader::deserialize(&header_bytes)
                .with_context(|| format!("Failed to deserialize header at position {}", offset))?;

            let block_hash = header.block_hash()?;

            headers.push(ScannedHeader {
                block_hash,
                prev_hash: header.prev_blockhash,
                time: header.time,
                file_offset: offset,
                block_size,
            });

            // Skip the rest of the block (transactions)
            let remaining = block_size as u64 - 80;
            self.reader.seek(SeekFrom::Current(remaining as i64))?;
            self.position += remaining;
        }

        Ok(headers)
    }

    /// Read a full block at a specific file offset.
    ///
    /// Seeks to `offset`, reads magic + size + block data, and deserializes.
    /// Used in pass 2 of the two-pass indexer to read blocks in chain order.
    pub fn read_block_at(&mut self, offset: u64) -> Result<Block> {
        self.reader.seek(SeekFrom::Start(offset))?;
        self.position = offset;
        self.last_block_start = offset;

        // Read magic
        let magic = self
            .reader
            .read_u32::<BigEndian>()
            .with_context(|| format!("Failed to read magic at offset {}", offset))?;
        self.position += 4;

        if magic != self.magic_bytes {
            anyhow::bail!(
                "Invalid magic bytes at offset {}: expected 0x{:08X}, found 0x{:08X}",
                offset,
                self.magic_bytes,
                magic
            );
        }

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

        Block::deserialize(&block_data)
            .with_context(|| format!("Failed to deserialize block at offset {}", offset))
    }
}

/// A scanned block header with its file location.
///
/// Produced by `BlockFileReader::scan_headers()` during pass 1 of indexing.
/// Contains just enough data to build the chain and re-locate the block
/// for full reading in pass 2.
#[derive(Debug, Clone)]
pub struct ScannedHeader {
    /// Block hash (X11 of header, internal byte order)
    pub block_hash: [u8; 32],
    /// Previous block hash (internal byte order)
    pub prev_hash: [u8; 32],
    /// Block timestamp (for tie-breaking if needed)
    pub time: u32,
    /// Byte offset of this block's magic bytes in the blk file
    pub file_offset: u64,
    /// Full block size in bytes (excluding magic + size prefix)
    pub block_size: u32,
}
