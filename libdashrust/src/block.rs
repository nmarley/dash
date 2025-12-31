// Copyright (c) 2025 The Dash Core developers
// Distributed under the MIT software license

//! Block and block header types

use crate::error::Result;
use crate::serialize::{read_compact_size, write_compact_size};
use crate::transaction::Transaction;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Block header
///
/// Corresponds to CBlockHeader in Dash Core.
/// Always exactly 80 bytes when serialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    /// Block version
    pub version: i32,
    /// Previous block hash
    pub prev_blockhash: [u8; 32],
    /// Merkle root hash
    pub merkle_root: [u8; 32],
    /// Block timestamp
    pub time: u32,
    /// Difficulty target (compact representation)
    pub bits: u32,
    /// Nonce
    pub nonce: u32,
}

impl BlockHeader {
    /// Serialize block header to bytes
    ///
    /// Always produces exactly 80 bytes
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(80);

        buf.write_i32::<LittleEndian>(self.version)?;
        buf.write_all(&self.prev_blockhash)?;
        buf.write_all(&self.merkle_root)?;
        buf.write_u32::<LittleEndian>(self.time)?;
        buf.write_u32::<LittleEndian>(self.bits)?;
        buf.write_u32::<LittleEndian>(self.nonce)?;

        debug_assert_eq!(buf.len(), 80, "Block header must be exactly 80 bytes");
        Ok(buf)
    }

    /// Deserialize block header from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 80 {
            return Err(crate::error::DashError::Serialization(
                "Block header must be at least 80 bytes".to_string(),
            ));
        }

        let mut cursor = std::io::Cursor::new(data);

        let version = cursor.read_i32::<LittleEndian>()?;
        
        let mut prev_blockhash = [0u8; 32];
        cursor.read_exact(&mut prev_blockhash)?;
        
        let mut merkle_root = [0u8; 32];
        cursor.read_exact(&mut merkle_root)?;
        
        let time = cursor.read_u32::<LittleEndian>()?;
        let bits = cursor.read_u32::<LittleEndian>()?;
        let nonce = cursor.read_u32::<LittleEndian>()?;

        Ok(BlockHeader {
            version,
            prev_blockhash,
            merkle_root,
            time,
            bits,
            nonce,
        })
    }
}

/// Block - header plus transactions
///
/// Corresponds to CBlock in Dash Core
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,
    /// Transactions in this block
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Serialize block to bytes
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Serialize header (80 bytes)
        let header_bytes = self.header.serialize()?;
        buf.write_all(&header_bytes)?;

        // Serialize transaction count
        write_compact_size(&mut buf, self.transactions.len() as u64)?;

        // Serialize each transaction
        for tx in &self.transactions {
            let tx_bytes = tx.serialize()?;
            buf.write_all(&tx_bytes)?;
        }

        Ok(buf)
    }

    /// Deserialize block from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read header (80 bytes)
        let mut header_bytes = [0u8; 80];
        cursor.read_exact(&mut header_bytes)?;
        let header = BlockHeader::deserialize(&header_bytes)?;

        // Read transaction count
        let tx_count = read_compact_size(&mut cursor)?;

        // Read transactions
        let mut transactions = Vec::with_capacity(tx_count as usize);
        for _ in 0..tx_count {
            // Read the rest of the data for this transaction
            let remaining = data.len() - cursor.position() as usize;
            let mut temp_buf = vec![0u8; remaining];
            cursor.read_exact(&mut temp_buf)?;
            
            // Try to deserialize transaction
            let tx = Transaction::deserialize(&temp_buf)?;
            
            // Calculate how many bytes were consumed
            let tx_bytes = tx.serialize()?;
            cursor.set_position(cursor.position() - (remaining - tx_bytes.len()) as u64);
            
            transactions.push(tx);
        }

        Ok(Block {
            header,
            transactions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx_type::DashTxType;

    #[test]
    fn test_block_header_size() {
        let header = BlockHeader {
            version: 2,
            prev_blockhash: [0u8; 32],
            merkle_root: [1u8; 32],
            time: 1234567890,
            bits: 0x1d00ffff,
            nonce: 42,
        };

        let bytes = header.serialize().unwrap();
        assert_eq!(bytes.len(), 80, "Block header must be exactly 80 bytes");
    }

    #[test]
    fn test_block_header_roundtrip() {
        let header = BlockHeader {
            version: 2,
            prev_blockhash: [0xABu8; 32],
            merkle_root: [0xCDu8; 32],
            time: 1234567890,
            bits: 0x1d00ffff,
            nonce: 12345,
        };

        let bytes = header.serialize().unwrap();
        let decoded = BlockHeader::deserialize(&bytes).unwrap();

        assert_eq!(header, decoded);
    }

    #[test]
    fn test_block_with_transactions() {
        use crate::transaction::{OutPoint, TxIn, TxOut};

        let header = BlockHeader {
            version: 2,
            prev_blockhash: [0u8; 32],
            merkle_root: [0u8; 32],
            time: 1234567890,
            bits: 0x1d00ffff,
            nonce: 0,
        };

        let transactions = vec![
            Transaction {
                version: 2,
                tx_type: DashTxType::Normal,
                inputs: vec![TxIn {
                    previous_output: OutPoint {
                        hash: [0u8; 32],
                        n: u32::MAX,
                    },
                    script_sig: vec![0x01, 0x02, 0x03],
                    sequence: 0xFFFFFFFF,
                }],
                outputs: vec![TxOut {
                    value: 5000000000,
                    script_pubkey: vec![0x76, 0xa9],
                }],
                lock_time: 0,
                extra_payload: None,
            },
        ];

        let block = Block {
            header,
            transactions,
        };

        let bytes = block.serialize().unwrap();
        let decoded = Block::deserialize(&bytes).unwrap();

        assert_eq!(block, decoded);
        assert_eq!(decoded.transactions.len(), 1);
        assert_eq!(decoded.transactions[0].inputs.len(), 1);
    }

    #[test]
    fn test_empty_block() {
        let header = BlockHeader {
            version: 2,
            prev_blockhash: [0u8; 32],
            merkle_root: [0u8; 32],
            time: 0,
            bits: 0,
            nonce: 0,
        };

        let block = Block {
            header,
            transactions: vec![],
        };

        let bytes = block.serialize().unwrap();
        let decoded = Block::deserialize(&bytes).unwrap();

        assert_eq!(block, decoded);
        assert_eq!(decoded.transactions.len(), 0);
    }
}
