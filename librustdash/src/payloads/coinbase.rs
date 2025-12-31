//! Coinbase transaction payload (CbTx)

use crate::error::Result;
use crate::serialize::{read_compact_size, write_compact_size};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Coinbase transaction payload
///
/// Corresponds to CCbTx in Dash Core (src/evo/cbtx.h)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CbTx {
    /// Version of the coinbase payload
    pub version: u16,
    /// Block height
    pub height: i32,
    /// Merkle root of masternode list
    pub merkle_root_mn_list: [u8; 32],
    /// Merkle root of quorums (v2+)
    pub merkle_root_quorums: Option<[u8; 32]>,
    /// Best chainlock height difference (v3+)
    pub best_cl_height_diff: Option<u32>,
    /// Best chainlock signature (v3+, 96 bytes BLS signature)
    pub best_cl_signature: Option<Vec<u8>>,
    /// Credit pool balance (v3+)
    pub credit_pool_balance: Option<i64>,
}

impl CbTx {
    /// Serialize coinbase payload
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        buf.write_u16::<LittleEndian>(self.version)?;
        buf.write_i32::<LittleEndian>(self.height)?;
        buf.write_all(&self.merkle_root_mn_list)?;

        // Version 2+ includes quorum merkle root
        if self.version >= 2 {
            if let Some(ref quorum_root) = self.merkle_root_quorums {
                buf.write_all(quorum_root)?;
            } else {
                return Err(crate::error::DashError::Serialization(
                    "CbTx v2+ requires merkle_root_quorums".to_string(),
                ));
            }

            // Version 3+ includes chainlock and credit pool
            if self.version >= 3 {
                let height_diff = self.best_cl_height_diff.ok_or_else(|| {
                    crate::error::DashError::Serialization(
                        "CbTx v3+ requires best_cl_height_diff".to_string(),
                    )
                })?;
                write_compact_size(&mut buf, height_diff as u64)?;

                let signature = self.best_cl_signature.as_ref().ok_or_else(|| {
                    crate::error::DashError::Serialization(
                        "CbTx v3+ requires best_cl_signature".to_string(),
                    )
                })?;

                // BLS signatures are 96 bytes
                if signature.len() != 96 {
                    return Err(crate::error::DashError::InvalidBlsSignature);
                }
                buf.write_all(signature)?;

                let balance = self.credit_pool_balance.ok_or_else(|| {
                    crate::error::DashError::Serialization(
                        "CbTx v3+ requires credit_pool_balance".to_string(),
                    )
                })?;
                buf.write_i64::<LittleEndian>(balance)?;
            }
        }

        Ok(buf)
    }

    /// Deserialize coinbase payload
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        let version = cursor.read_u16::<LittleEndian>()?;
        let height = cursor.read_i32::<LittleEndian>()?;

        let mut merkle_root_mn_list = [0u8; 32];
        cursor.read_exact(&mut merkle_root_mn_list)?;

        let merkle_root_quorums = if version >= 2 {
            let mut hash = [0u8; 32];
            cursor.read_exact(&mut hash)?;
            Some(hash)
        } else {
            None
        };

        let (best_cl_height_diff, best_cl_signature, credit_pool_balance) = if version >= 3 {
            let diff = read_compact_size(&mut cursor)? as u32;

            let mut sig = vec![0u8; 96]; // BLS signature size
            cursor.read_exact(&mut sig)?;

            let balance = cursor.read_i64::<LittleEndian>()?;

            (Some(diff), Some(sig), Some(balance))
        } else {
            (None, None, None)
        };

        Ok(CbTx {
            version,
            height,
            merkle_root_mn_list,
            merkle_root_quorums,
            best_cl_height_diff,
            best_cl_signature,
            credit_pool_balance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbtx_v1_roundtrip() {
        let cbtx = CbTx {
            version: 1,
            height: 12345,
            merkle_root_mn_list: [0xAAu8; 32],
            merkle_root_quorums: None,
            best_cl_height_diff: None,
            best_cl_signature: None,
            credit_pool_balance: None,
        };

        let bytes = cbtx.serialize().unwrap();
        let decoded = CbTx::deserialize(&bytes).unwrap();

        assert_eq!(cbtx, decoded);
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.height, 12345);
    }

    #[test]
    fn test_cbtx_v2_roundtrip() {
        let cbtx = CbTx {
            version: 2,
            height: 54321,
            merkle_root_mn_list: [0xBBu8; 32],
            merkle_root_quorums: Some([0xCCu8; 32]),
            best_cl_height_diff: None,
            best_cl_signature: None,
            credit_pool_balance: None,
        };

        let bytes = cbtx.serialize().unwrap();
        let decoded = CbTx::deserialize(&bytes).unwrap();

        assert_eq!(cbtx, decoded);
        assert_eq!(decoded.version, 2);
        assert!(decoded.merkle_root_quorums.is_some());
    }

    #[test]
    fn test_cbtx_v3_roundtrip() {
        let cbtx = CbTx {
            version: 3,
            height: 99999,
            merkle_root_mn_list: [0xDDu8; 32],
            merkle_root_quorums: Some([0xEEu8; 32]),
            best_cl_height_diff: Some(10),
            best_cl_signature: Some(vec![0xFFu8; 96]), // 96-byte BLS signature
            credit_pool_balance: Some(1234567890),
        };

        let bytes = cbtx.serialize().unwrap();
        let decoded = CbTx::deserialize(&bytes).unwrap();

        assert_eq!(cbtx, decoded);
        assert_eq!(decoded.version, 3);
        assert!(decoded.merkle_root_quorums.is_some());
        assert!(decoded.best_cl_signature.is_some());
        assert_eq!(decoded.credit_pool_balance, Some(1234567890));
    }

    #[test]
    fn test_cbtx_v2_missing_quorum_root() {
        let cbtx = CbTx {
            version: 2,
            height: 100,
            merkle_root_mn_list: [0u8; 32],
            merkle_root_quorums: None, // Should be Some for v2
            best_cl_height_diff: None,
            best_cl_signature: None,
            credit_pool_balance: None,
        };

        assert!(cbtx.serialize().is_err());
    }

    #[test]
    fn test_cbtx_v3_invalid_signature_size() {
        let cbtx = CbTx {
            version: 3,
            height: 100,
            merkle_root_mn_list: [0u8; 32],
            merkle_root_quorums: Some([0u8; 32]),
            best_cl_height_diff: Some(0),
            best_cl_signature: Some(vec![0u8; 48]), // Wrong size (should be 96)
            credit_pool_balance: Some(0),
        };

        assert!(cbtx.serialize().is_err());
    }
}
