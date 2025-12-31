// Copyright (c) 2025 The Dash Core developers
// Distributed under the MIT software license

//! Asset lock/unlock transaction payloads

use crate::error::Result;
use crate::serialize::{read_compact_size, write_compact_size};
use crate::transaction::TxOut;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Asset lock transaction payload
///
/// Corresponds to CAssetLockPayload in Dash Core (src/evo/assetlocktx.h)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLockPayload {
    /// Version
    pub version: u8,
    /// Credit outputs being locked
    pub credit_outputs: Vec<TxOut>,
}

impl AssetLockPayload {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        buf.write_u8(self.version)?;
        
        // Serialize credit outputs
        write_compact_size(&mut buf, self.credit_outputs.len() as u64)?;
        for output in &self.credit_outputs {
            output.serialize(&mut buf)?;
        }

        Ok(buf)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        let version = cursor.read_u8()?;
        
        let output_count = read_compact_size(&mut cursor)?;
        let mut credit_outputs = Vec::with_capacity(output_count as usize);
        for _ in 0..output_count {
            credit_outputs.push(TxOut::deserialize(&mut cursor)?);
        }

        Ok(AssetLockPayload {
            version,
            credit_outputs,
        })
    }
}

/// Asset unlock transaction payload
///
/// Corresponds to CAssetUnlockPayload in Dash Core (src/evo/assetlocktx.h)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetUnlockPayload {
    /// Version
    pub version: u8,
    /// Index
    pub index: u64,
    /// Fee
    pub fee: u32,
    /// Requested height
    pub requested_height: u32,
    /// Quorum hash
    pub quorum_hash: [u8; 32],
    /// Quorum signature (96 bytes BLS signature)
    pub quorum_sig: Vec<u8>,
}

impl AssetUnlockPayload {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        buf.write_u8(self.version)?;
        buf.write_u64::<LittleEndian>(self.index)?;
        buf.write_u32::<LittleEndian>(self.fee)?;
        buf.write_u32::<LittleEndian>(self.requested_height)?;
        buf.write_all(&self.quorum_hash)?;
        
        // BLS signatures are 96 bytes
        if self.quorum_sig.len() != 96 {
            return Err(crate::error::DashError::InvalidBlsSignature);
        }
        buf.write_all(&self.quorum_sig)?;

        Ok(buf)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        let version = cursor.read_u8()?;
        let index = cursor.read_u64::<LittleEndian>()?;
        let fee = cursor.read_u32::<LittleEndian>()?;
        let requested_height = cursor.read_u32::<LittleEndian>()?;
        
        let mut quorum_hash = [0u8; 32];
        cursor.read_exact(&mut quorum_hash)?;
        
        let mut quorum_sig = vec![0u8; 96];
        cursor.read_exact(&mut quorum_sig)?;

        Ok(AssetUnlockPayload {
            version,
            index,
            fee,
            requested_height,
            quorum_hash,
            quorum_sig,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_lock_roundtrip() {
        let payload = AssetLockPayload {
            version: 1,
            credit_outputs: vec![
                TxOut {
                    value: 1000000,
                    script_pubkey: vec![0x76, 0xa9],
                },
                TxOut {
                    value: 500000,
                    script_pubkey: vec![0x88, 0xac],
                },
            ],
        };

        let bytes = payload.serialize().unwrap();
        let decoded = AssetLockPayload::deserialize(&bytes).unwrap();

        assert_eq!(payload, decoded);
        assert_eq!(decoded.credit_outputs.len(), 2);
    }

    #[test]
    fn test_asset_lock_empty_outputs() {
        let payload = AssetLockPayload {
            version: 1,
            credit_outputs: vec![],
        };

        let bytes = payload.serialize().unwrap();
        let decoded = AssetLockPayload::deserialize(&bytes).unwrap();

        assert_eq!(payload, decoded);
        assert_eq!(decoded.credit_outputs.len(), 0);
    }

    #[test]
    fn test_asset_unlock_roundtrip() {
        let payload = AssetUnlockPayload {
            version: 1,
            index: 123456,
            fee: 1000,
            requested_height: 999999,
            quorum_hash: [0xAAu8; 32],
            quorum_sig: vec![0xBBu8; 96],
        };

        let bytes = payload.serialize().unwrap();
        let decoded = AssetUnlockPayload::deserialize(&bytes).unwrap();

        assert_eq!(payload, decoded);
        assert_eq!(decoded.index, 123456);
        assert_eq!(decoded.fee, 1000);
        assert_eq!(decoded.quorum_sig.len(), 96);
    }

    #[test]
    fn test_asset_unlock_invalid_signature_size() {
        let payload = AssetUnlockPayload {
            version: 1,
            index: 0,
            fee: 0,
            requested_height: 0,
            quorum_hash: [0u8; 32],
            quorum_sig: vec![0u8; 48], // Wrong size
        };

        assert!(payload.serialize().is_err());
    }
}
