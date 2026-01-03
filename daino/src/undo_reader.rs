//! Undo file reader for Dash rev*.dat files
//!
//! Reads undo data from Dash Core's undo files (rev00000.dat, etc.)

use crate::block_reader::Network;
use crate::undo::CBlockUndo;
use anyhow::{Context, Result};
use byteorder::{BigEndian, ReadBytesExt};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Simple undo file reader
pub struct UndoFileReader {
    reader: BufReader<File>,
    magic_bytes: u32,
    position: u64,
    last_undo_start: u64,
}

impl UndoFileReader {
    /// Create a new undo file reader
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

    /// Read the next undo block from the file
    ///
    /// Note: The prev_block_hash is required to verify the checksum, but it's not stored
    /// in the file. It must be obtained from the corresponding block's previous block hash.
    /// If you don't have access to the block data, pass None to skip checksum verification.
    pub fn read_next_undo(
        &mut self,
        prev_block_hash: Option<[u8; 32]>,
    ) -> Result<Option<CBlockUndo>> {
        // Record the start position of this undo block
        self.last_undo_start = self.position;

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
                self.last_undo_start,
                self.magic_bytes,
                magic
            );
        }

        self.position += 4;

        // Read size of CBlockUndo (as uint32 little-endian, NOT varint!)
        let size = self
            .reader
            .read_u32::<byteorder::LittleEndian>()
            .context("Failed to read undo block size")?;
        self.position += 4;

        // Read undo data
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

        // Verify checksum if prev_block_hash is provided
        if let Some(prev_hash) = prev_block_hash {
            // Compute checksum: SHA256(prevBlockHash + CBlockUndo)
            let mut hasher = Sha256::new();
            hasher.update(&prev_hash);
            hasher.update(&undo_data);
            let computed_hash = hasher.finalize();

            if computed_hash.as_slice() != &stored_hash {
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

    /// Get current position in the file (after last read)
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the start position of the last undo block read
    pub fn last_undo_start(&self) -> u64 {
        self.last_undo_start
    }
}
