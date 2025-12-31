//! Serialization utilities for Dash primitives

use crate::error::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Write a CompactSize value (variable-length integer)
///
/// CompactSize format (matches Bitcoin/Dash):
/// - 0-252: 1 byte
/// - 253-65535: 0xFD + 2 bytes (little-endian)
/// - 65536-2^32-1: 0xFE + 4 bytes (little-endian)
/// - 2^32-2^64-1: 0xFF + 8 bytes (little-endian)
pub fn write_compact_size<W: Write>(writer: &mut W, value: u64) -> Result<()> {
    match value {
        0..=252 => {
            writer.write_u8(value as u8)?;
        }
        253..=0xFFFF => {
            writer.write_u8(0xFD)?;
            writer.write_u16::<LittleEndian>(value as u16)?;
        }
        0x10000..=0xFFFFFFFF => {
            writer.write_u8(0xFE)?;
            writer.write_u32::<LittleEndian>(value as u32)?;
        }
        _ => {
            writer.write_u8(0xFF)?;
            writer.write_u64::<LittleEndian>(value)?;
        }
    }
    Ok(())
}

/// Read a CompactSize value
pub fn read_compact_size<R: Read>(reader: &mut R) -> Result<u64> {
    let first_byte = reader.read_u8()?;

    match first_byte {
        0..=252 => Ok(first_byte as u64),
        0xFD => Ok(reader.read_u16::<LittleEndian>()? as u64),
        0xFE => Ok(reader.read_u32::<LittleEndian>()? as u64),
        0xFF => Ok(reader.read_u64::<LittleEndian>()?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_size_roundtrip() {
        let test_cases = vec![
            (0, vec![0x00]),
            (252, vec![0xFC]),
            (253, vec![0xFD, 0xFD, 0x00]),
            (255, vec![0xFD, 0xFF, 0x00]),
            (65535, vec![0xFD, 0xFF, 0xFF]),
            (65536, vec![0xFE, 0x00, 0x00, 0x01, 0x00]),
            (0xFFFFFFFF, vec![0xFE, 0xFF, 0xFF, 0xFF, 0xFF]),
        ];

        for (value, expected_bytes) in test_cases {
            // Test write
            let mut buf = Vec::new();
            write_compact_size(&mut buf, value).unwrap();
            assert_eq!(buf, expected_bytes, "Failed for value {}", value);

            // Test read
            let mut cursor = &buf[..];
            let decoded = read_compact_size(&mut cursor).unwrap();
            assert_eq!(decoded, value, "Failed roundtrip for value {}", value);
        }
    }

    #[test]
    fn test_compact_size_edge_cases() {
        // Test boundary values
        let boundaries = vec![
            0u64, 1, 252, 253, 254, 255, 256, 65534, 65535, 65536, 0xFFFFFFFF,
        ];

        for value in boundaries {
            let mut buf = Vec::new();
            write_compact_size(&mut buf, value).unwrap();
            let decoded = read_compact_size(&mut &buf[..]).unwrap();
            assert_eq!(decoded, value);
        }
    }
}
