//! Undo file reader for Dash rev*.dat files
//!
//! Reads undo data from Dash Core's undo files (rev00000.dat, etc.)
//! Supports both sequential reading and seek-based random access.

use crate::block_reader::Network;
use crate::undo::CBlockUndo;
use anyhow::{Context, Result};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Undo file reader with sequential and seek-based access.
pub struct UndoFileReader {
    reader: BufReader<File>,
    magic_bytes: u32,
    position: u64,
    last_undo_start: u64,
}

impl UndoFileReader {
    /// Create a new undo file reader.
    pub fn new<P: AsRef<Path>>(path: P, network: Network) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open undo file: {:?}", path.as_ref()))?;
        let reader = BufReader::new(file);

        Ok(Self {
            reader,
            magic_bytes: network.magic_bytes(),
            position: 0,
            last_undo_start: 0,
        })
    }

    /// Read the next undo block from the file (sequential).
    ///
    /// The prev_block_hash is required to verify the checksum. It is the
    /// hash of the block's *parent* (i.e., `block.header.prev_blockhash`),
    /// not the block's own hash. Pass None to skip checksum verification.
    pub fn read_next_undo(
        &mut self,
        prev_block_hash: Option<[u8; 32]>,
    ) -> Result<Option<CBlockUndo>> {
        self.last_undo_start = self.position;
        self.read_undo_inner(prev_block_hash)
    }

    /// Read an undo block at a specific file offset (seek-based).
    ///
    /// Seeks to `offset`, reads magic + size + payload + checksum, and
    /// deserializes. Analogous to `BlockFileReader::read_block_at()`.
    pub fn read_undo_at(
        &mut self,
        offset: u64,
        prev_block_hash: Option<[u8; 32]>,
    ) -> Result<CBlockUndo> {
        self.reader.seek(SeekFrom::Start(offset))?;
        self.position = offset;
        self.last_undo_start = offset;

        self.read_undo_inner(prev_block_hash)?
            .ok_or_else(|| anyhow::anyhow!("Unexpected EOF at undo offset {}", offset))
    }

    /// Shared read logic for both sequential and seek-based access.
    fn read_undo_inner(&mut self, prev_block_hash: Option<[u8; 32]>) -> Result<Option<CBlockUndo>> {
        // Read magic bytes (big-endian / network byte order)
        let magic = match self.reader.read_u32::<BigEndian>() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        if magic != self.magic_bytes {
            anyhow::bail!(
                "Invalid magic bytes at position {}: expected 0x{:08X}, found 0x{:08X}",
                self.last_undo_start,
                self.magic_bytes,
                magic
            );
        }

        self.position += 4;

        // Read size of CBlockUndo (uint32 little-endian)
        let size = self
            .reader
            .read_u32::<LittleEndian>()
            .context("Failed to read undo block size")?;
        self.position += 4;

        // Read undo payload
        let mut undo_data = vec![0u8; size as usize];
        self.reader
            .read_exact(&mut undo_data)
            .context("Failed to read undo data")?;
        self.position += size as u64;

        // Read stored checksum (32 bytes)
        let mut stored_hash = [0u8; 32];
        self.reader
            .read_exact(&mut stored_hash)
            .context("Failed to read checksum")?;
        self.position += 32;

        // Verify checksum if prev_block_hash is provided.
        // Dash Core uses double-SHA256: SHA256(SHA256(prev_block_hash || undo_data))
        // See HashWriter::GetHash() in src/hash.h
        if let Some(prev_hash) = prev_block_hash {
            let mut hasher = Sha256::new();
            hasher.update(prev_hash);
            hasher.update(&undo_data);
            let first = hasher.finalize();

            let mut hasher2 = Sha256::new();
            hasher2.update(first);
            let computed_hash = hasher2.finalize();

            if computed_hash.as_slice() != stored_hash {
                anyhow::bail!(
                    "Checksum mismatch at position {}: expected {}, got {}",
                    self.last_undo_start,
                    hex::encode(stored_hash),
                    hex::encode(computed_hash)
                );
            }
        }

        // Deserialize CBlockUndo
        let block_undo = CBlockUndo::deserialize(&mut &undo_data[..]).with_context(|| {
            format!(
                "Failed to deserialize undo block at position {}",
                self.last_undo_start
            )
        })?;

        Ok(Some(block_undo))
    }

    /// Get current position in the file (after last read).
    #[allow(dead_code)]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the start position of the last undo block read.
    pub fn last_undo_start(&self) -> u64 {
        self.last_undo_start
    }
}

/// Scan a rev*.dat file and return the byte offset of each undo entry.
///
/// This is a fast scan that reads only magic + size headers and skips
/// the payload and checksum. No deserialization is performed. The
/// returned offsets are in file order (which is chain-height order
/// within each rev file).
///
/// Each offset points to the start of the entry (the magic bytes),
/// suitable for passing to `UndoFileReader::read_undo_at()`.
pub fn scan_undo_offsets<P: AsRef<Path>>(path: P, network: Network) -> Result<Vec<u64>> {
    let file = File::open(path.as_ref())
        .with_context(|| format!("Failed to open undo file: {:?}", path.as_ref()))?;
    let mut reader = BufReader::new(file);
    let magic_bytes = network.magic_bytes();
    let mut offsets = Vec::new();
    let mut position: u64 = 0;

    loop {
        let entry_start = position;

        // Read magic bytes
        let magic = match reader.read_u32::<BigEndian>() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        position += 4;

        if magic != magic_bytes {
            anyhow::bail!(
                "Invalid magic bytes in {:?} at position {}: expected 0x{:08X}, found 0x{:08X}",
                path.as_ref(),
                entry_start,
                magic_bytes,
                magic
            );
        }

        // Read payload size
        let size = reader
            .read_u32::<LittleEndian>()
            .with_context(|| format!("Failed to read undo size at position {}", entry_start))?;
        position += 4;

        offsets.push(entry_start);

        // Skip payload + 32-byte checksum
        let skip = size as u64 + 32;
        reader.seek(SeekFrom::Current(skip as i64))?;
        position += skip;
    }

    Ok(offsets)
}
