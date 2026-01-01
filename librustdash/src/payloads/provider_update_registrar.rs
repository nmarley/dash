//! Provider Update Registrar Transaction (ProUpRegTx)
//!
//! This module implements the ProUpRegTx special transaction payload type,
//! used to update a masternode's operator key, voting key, and payout script.
//!
//! # Overview
//!
//! ProUpRegTx allows a masternode owner to update the masternode's:
//! - Operator public key (BLS)
//! - Voting key ID
//! - Payout script
//!
//! Unlike ProUpServTx, this transaction does NOT update the service address.
//! It's signed with an ECDSA signature from the owner key (not BLS).
//!
//! # Transaction Type
//!
//! - **Transaction Type**: `TRANSACTION_PROVIDER_UPDATE_REGISTRAR` (3)
//! - **Versions**: 1 (Legacy BLS), 2 (Basic BLS)
//!
//! # Use Cases
//!
//! - Rotating operator keys for security
//! - Changing voting key delegation
//! - Updating payout address
//!
//! # Serialization Format
//!
//! ```text
//! version           (u16)           - Message version
//! pro_tx_hash       (32 bytes)      - Hash of the initial ProRegTx
//! mode              (u16)           - Currently only 0 supported
//! pubkey_operator   (48 bytes)      - BLS operator public key
//! key_id_voting     (20 bytes)      - CKeyID for voting
//! script_payout     (CompactSize+)  - Payout script
//! inputs_hash       (32 bytes)      - Hash of transaction inputs
//! sig               (CompactSize+)  - ECDSA signature from owner key
//! ```
//!
//! # Reference
//!
//! Dash Core implementation: `src/evo/providertx.h` (CProUpRegTx)

use crate::bls::BlsPublicKey;
use crate::error::Result;
use crate::serialize::{read_compact_size, write_compact_size};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Provider Update Registrar Transaction payload
///
/// Used to update a masternode's operator public key, voting key ID,
/// and payout script. Unlike ProUpServTx, this does NOT update the service address.
///
/// # Fields
///
/// - `version`: Message version (1 = Legacy BLS, 2 = Basic BLS)
/// - `pro_tx_hash`: Hash of the original ProRegTx that registered this masternode
/// - `mode`: Operating mode (currently only 0 is supported)
/// - `pubkey_operator`: BLS public key for the operator
/// - `key_id_voting`: 20-byte key ID for voting (CKeyID/uint160)
/// - `script_payout`: Bitcoin script for operator/owner payouts
/// - `inputs_hash`: SHA256 hash of transaction inputs for replay protection
/// - `sig`: ECDSA signature from the owner key (variable length, typically 65 bytes)
///
/// # Signature
///
/// Unlike most provider transactions which use BLS signatures, ProUpRegTx uses
/// an ECDSA signature from the owner key to prove ownership.
///
/// # Example
///
/// ```
/// use librustdash::{ProUpRegTx, BlsPublicKey};
///
/// let tx = ProUpRegTx {
///     version: 1,
///     pro_tx_hash: [0xAA; 32],
///     mode: 0,
///     pubkey_operator: BlsPublicKey { data: vec![0xBB; 48] },
///     key_id_voting: [0xCC; 20],
///     script_payout: vec![0x76, 0xa9], // Example P2PKH script prefix
///     inputs_hash: [0xDD; 32],
///     sig: vec![0xEE; 65], // ECDSA signature
/// };
///
/// let bytes = tx.serialize().unwrap();
/// let decoded = ProUpRegTx::deserialize(&bytes).unwrap();
/// assert_eq!(tx, decoded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProUpRegTx {
    /// Message version (1 = Legacy BLS, 2 = Basic BLS)
    pub version: u16,
    /// Hash of the original ProRegTx
    pub pro_tx_hash: [u8; 32],
    /// Operating mode (currently only 0 supported)
    pub mode: u16,
    /// BLS operator public key
    pub pubkey_operator: BlsPublicKey,
    /// Voting key ID (CKeyID/uint160)
    pub key_id_voting: [u8; 20],
    /// Payout script (variable length)
    pub script_payout: Vec<u8>,
    /// Hash of transaction inputs (replay protection)
    pub inputs_hash: [u8; 32],
    /// ECDSA signature from owner key (variable length)
    pub sig: Vec<u8>,
}

impl ProUpRegTx {
    /// Serialize the ProUpRegTx payload
    ///
    /// Serializes to bytes matching Dash Core's binary format.
    ///
    /// # Format
    ///
    /// ```text
    /// version           (2 bytes)       - u16 little-endian
    /// pro_tx_hash       (32 bytes)      - As-is
    /// mode              (2 bytes)       - u16 little-endian
    /// pubkey_operator   (48 bytes)      - BLS public key
    /// key_id_voting     (20 bytes)      - As-is
    /// script_payout     (var)           - CompactSize length + bytes
    /// inputs_hash       (32 bytes)      - As-is
    /// sig               (var)           - CompactSize length + ECDSA signature
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the BLS operator public key is not exactly 48 bytes.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version
        buf.write_u16::<LittleEndian>(self.version)?;

        // Write pro_tx_hash
        buf.write_all(&self.pro_tx_hash)?;

        // Write mode
        buf.write_u16::<LittleEndian>(self.mode)?;

        // Write BLS operator public key (validate size first)
        let pubkey_bytes = self.pubkey_operator.serialize()?;
        buf.write_all(&pubkey_bytes)?;

        // Write voting key ID
        buf.write_all(&self.key_id_voting)?;

        // Write payout script with CompactSize prefix
        write_compact_size(&mut buf, self.script_payout.len() as u64)?;
        buf.write_all(&self.script_payout)?;

        // Write inputs_hash
        buf.write_all(&self.inputs_hash)?;

        // Write signature with CompactSize prefix
        write_compact_size(&mut buf, self.sig.len() as u64)?;
        buf.write_all(&self.sig)?;

        Ok(buf)
    }

    /// Deserialize a ProUpRegTx payload from bytes
    ///
    /// Reads the binary format produced by Dash Core.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Data is too short
    /// - BLS public key is malformed (not 48 bytes)
    /// - CompactSize values are invalid
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version
        let version = cursor.read_u16::<LittleEndian>()?;

        // Read pro_tx_hash
        let mut pro_tx_hash = [0u8; 32];
        cursor.read_exact(&mut pro_tx_hash)?;

        // Read mode
        let mode = cursor.read_u16::<LittleEndian>()?;

        // Read BLS operator public key (48 bytes)
        let mut pubkey_data = vec![0u8; 48];
        cursor.read_exact(&mut pubkey_data)?;
        let pubkey_operator = BlsPublicKey { data: pubkey_data };

        // Read voting key ID (20 bytes)
        let mut key_id_voting = [0u8; 20];
        cursor.read_exact(&mut key_id_voting)?;

        // Read payout script
        let script_len = read_compact_size(&mut cursor)? as usize;
        let mut script_payout = vec![0u8; script_len];
        cursor.read_exact(&mut script_payout)?;

        // Read inputs_hash
        let mut inputs_hash = [0u8; 32];
        cursor.read_exact(&mut inputs_hash)?;

        // Read signature
        let sig_len = read_compact_size(&mut cursor)? as usize;
        let mut sig = vec![0u8; sig_len];
        cursor.read_exact(&mut sig)?;

        Ok(ProUpRegTx {
            version,
            pro_tx_hash,
            mode,
            pubkey_operator,
            key_id_voting,
            script_payout,
            inputs_hash,
            sig,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proupregtx_v1_roundtrip() {
        let tx = ProUpRegTx {
            version: 1,
            pro_tx_hash: [0xAA; 32],
            mode: 0,
            pubkey_operator: BlsPublicKey {
                data: vec![0xBB; 48],
            },
            key_id_voting: [0xCC; 20],
            script_payout: vec![0x76, 0xa9],
            inputs_hash: [0xDD; 32],
            sig: vec![0xEE; 65], // ECDSA signature
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_proupregtx_v2_basic_bls() {
        let tx = ProUpRegTx {
            version: 2, // BasicBLS
            pro_tx_hash: [0; 32],
            mode: 0,
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            script_payout: vec![],
            inputs_hash: [0; 32],
            sig: vec![0; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.version, decoded.version);
        assert_eq!(decoded.version, 2);
    }

    #[test]
    fn test_proupregtx_empty_script() {
        let tx = ProUpRegTx {
            version: 1,
            pro_tx_hash: [0; 32],
            mode: 0,
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            script_payout: vec![],
            inputs_hash: [0; 32],
            sig: vec![],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.script_payout.len(), decoded.script_payout.len());
        assert_eq!(decoded.script_payout.len(), 0);
    }

    #[test]
    fn test_proupregtx_variable_signature() {
        // Test different signature lengths
        for sig_len in [0, 64, 65, 70, 71] {
            let tx = ProUpRegTx {
                version: 1,
                pro_tx_hash: [0x11; 32],
                mode: 0,
                pubkey_operator: BlsPublicKey {
                    data: vec![0x22; 48],
                },
                key_id_voting: [0x33; 20],
                script_payout: vec![0x76, 0xa9],
                inputs_hash: [0x44; 32],
                sig: vec![0x55; sig_len],
            };
            let bytes = tx.serialize().unwrap();
            let decoded = ProUpRegTx::deserialize(&bytes).unwrap();
            assert_eq!(tx.sig.len(), decoded.sig.len());
        }
    }

    #[test]
    fn test_proupregtx_invalid_operator_key() {
        let tx = ProUpRegTx {
            version: 1,
            pro_tx_hash: [0; 32],
            mode: 0,
            pubkey_operator: BlsPublicKey {
                data: vec![0; 47], // Wrong size
            },
            key_id_voting: [0; 20],
            script_payout: vec![],
            inputs_hash: [0; 32],
            sig: vec![],
        };
        assert!(tx.serialize().is_err());
    }

    #[test]
    fn test_proupregtx_large_script() {
        // Test with a larger payout script
        let tx = ProUpRegTx {
            version: 1,
            pro_tx_hash: [0; 32],
            mode: 0,
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            script_payout: vec![0x76; 100], // 100 byte script
            inputs_hash: [0; 32],
            sig: vec![0; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.script_payout, decoded.script_payout);
    }
}
