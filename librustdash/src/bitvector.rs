//! BitVector type for LLMQ quorum commitments
//!
//! BitVectors are used to represent sets of boolean values efficiently,
//! particularly for tracking which masternode members signed or are valid
//! in LLMQ quorum commitments.
//!
//! Serialization format: CompactSize(byte_count) + packed bytes

use crate::error::Result;
use crate::serialize::{read_compact_size, write_compact_size};
use std::io::{Read, Write};

/// A bit vector for efficiently storing boolean arrays
///
/// Used in LLMQ quorum commitments to track signers and valid members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitVector {
    pub bits: Vec<bool>,
}

impl BitVector {
    /// Create a new empty BitVector
    pub fn new() -> Self {
        BitVector { bits: Vec::new() }
    }

    /// Create a BitVector with a specific size, all bits set to false
    pub fn with_size(size: usize) -> Self {
        BitVector {
            bits: vec![false; size],
        }
    }

    /// Get the number of bits
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Check if the BitVector is empty
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    /// Serialize the BitVector
    ///
    /// Format: CompactSize(byte_count) + packed bytes
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Calculate number of bytes needed
        let byte_count = self.bits.len().div_ceil(8);

        // Write byte count as CompactSize
        write_compact_size(&mut buf, byte_count as u64)?;

        // Pack bits into bytes
        let mut bytes = vec![0u8; byte_count];
        for (i, &bit) in self.bits.iter().enumerate() {
            if bit {
                let byte_index = i / 8;
                let bit_index = i % 8;
                bytes[byte_index] |= 1 << bit_index;
            }
        }

        buf.write_all(&bytes)?;
        Ok(buf)
    }

    /// Deserialize a BitVector from bytes
    ///
    /// Note: This reads the exact number of bits based on byte_count * 8
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read byte count
        let byte_count = read_compact_size(&mut cursor)? as usize;

        // Read packed bytes
        let mut bytes = vec![0u8; byte_count];
        cursor.read_exact(&mut bytes)?;

        // Unpack bits
        let mut bits = Vec::with_capacity(byte_count * 8);
        for byte in bytes {
            for bit_index in 0..8 {
                let bit = (byte & (1 << bit_index)) != 0;
                bits.push(bit);
            }
        }

        Ok(BitVector { bits })
    }

    /// Deserialize with a specific expected bit count
    ///
    /// This truncates the unpacked bits to the expected size
    pub fn deserialize_with_size(data: &[u8], expected_bits: usize) -> Result<Self> {
        let mut bv = Self::deserialize(data)?;
        bv.bits.truncate(expected_bits);
        Ok(bv)
    }
}

impl Default for BitVector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitvector_empty() {
        let bv = BitVector { bits: vec![] };
        let bytes = bv.serialize().unwrap();
        let decoded = BitVector::deserialize(&bytes).unwrap();
        assert_eq!(bv, decoded);
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_bitvector_roundtrip() {
        let bv = BitVector {
            bits: vec![true, false, true, false, true, true, false, false, true],
        };
        let bytes = bv.serialize().unwrap();
        let decoded = BitVector::deserialize(&bytes).unwrap();

        // Note: deserialize unpacks all 16 bits (2 bytes * 8)
        // So we need to truncate to original size
        let decoded_trimmed = BitVector {
            bits: decoded.bits[..9].to_vec(),
        };
        assert_eq!(bv, decoded_trimmed);
    }

    #[test]
    fn test_bitvector_byte_alignment() {
        // 10 bits should use 2 bytes
        let bv = BitVector {
            bits: vec![true; 10],
        };
        let bytes = bv.serialize().unwrap();
        // CompactSize(2) + 2 bytes of data
        assert_eq!(bytes.len(), 1 + 2); // 0x02 + 2 bytes
        assert_eq!(bytes[0], 0x02); // CompactSize encoding of 2
    }

    #[test]
    fn test_bitvector_all_true() {
        let bv = BitVector {
            bits: vec![true; 8],
        };
        let bytes = bv.serialize().unwrap();
        // CompactSize(1) + 1 byte of 0xFF
        assert_eq!(bytes, vec![0x01, 0xFF]);

        let decoded = BitVector::deserialize(&bytes).unwrap();
        assert_eq!(decoded.bits.len(), 8);
        assert!(decoded.bits.iter().all(|&b| b));
    }

    #[test]
    fn test_bitvector_all_false() {
        let bv = BitVector {
            bits: vec![false; 8],
        };
        let bytes = bv.serialize().unwrap();
        // CompactSize(1) + 1 byte of 0x00
        assert_eq!(bytes, vec![0x01, 0x00]);

        let decoded = BitVector::deserialize(&bytes).unwrap();
        assert_eq!(decoded.bits.len(), 8);
        assert!(decoded.bits.iter().all(|&b| !b));
    }

    #[test]
    fn test_bitvector_pattern() {
        // Alternating pattern: 10101010
        let bv = BitVector {
            bits: vec![false, true, false, true, false, true, false, true],
        };
        let bytes = bv.serialize().unwrap();
        // CompactSize(1) + 1 byte: 0b10101010 = 0xAA (LSB first)
        assert_eq!(bytes, vec![0x01, 0xAA]);
    }

    #[test]
    fn test_bitvector_multi_byte() {
        // 16 bits all true
        let bv = BitVector {
            bits: vec![true; 16],
        };
        let bytes = bv.serialize().unwrap();
        // CompactSize(2) + 2 bytes of 0xFF
        assert_eq!(bytes, vec![0x02, 0xFF, 0xFF]);
    }

    #[test]
    fn test_bitvector_partial_byte() {
        // 5 bits: true, false, true, true, false
        let bv = BitVector {
            bits: vec![true, false, true, true, false],
        };
        let bytes = bv.serialize().unwrap();
        // Should use 1 byte
        // Bits packed LSB first: bit0=1, bit1=0, bit2=1, bit3=1, bit4=0, rest=0
        // 0b00001101 = 0x0D
        assert_eq!(bytes, vec![0x01, 0x0D]);
    }

    #[test]
    fn test_bitvector_with_size() {
        let bv = BitVector::with_size(50);
        assert_eq!(bv.len(), 50);
        assert!(bv.bits.iter().all(|&b| !b));
    }

    #[test]
    fn test_bitvector_deserialize_with_size() {
        // Create a 10-bit vector
        let bv = BitVector {
            bits: vec![true; 10],
        };
        let bytes = bv.serialize().unwrap();

        // Deserialize with exact size
        let decoded = BitVector::deserialize_with_size(&bytes, 10).unwrap();
        assert_eq!(decoded.bits.len(), 10);
        assert!(decoded.bits.iter().all(|&b| b));
    }

    #[test]
    fn test_bitvector_new() {
        let bv = BitVector::new();
        assert!(bv.is_empty());
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_bitvector_default() {
        let bv = BitVector::default();
        assert!(bv.is_empty());
    }

    #[test]
    fn test_bitvector_large() {
        // Test with 400 bits (typical for LLMQ_400_60)
        let bv = BitVector {
            bits: vec![true; 400],
        };
        let bytes = bv.serialize().unwrap();

        // 400 bits = 50 bytes
        // CompactSize(50) + 50 bytes
        assert_eq!(bytes[0], 50); // CompactSize of 50
        assert_eq!(bytes.len(), 1 + 50);

        let decoded = BitVector::deserialize_with_size(&bytes, 400).unwrap();
        assert_eq!(decoded.bits.len(), 400);
        assert!(decoded.bits.iter().all(|&b| b));
    }
}
