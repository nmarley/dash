//! Provider Update Revoke Transaction (ProUpRevTx)
//!
//! This module implements the ProUpRevTx special transaction payload type,
//! used to revoke a masternode's operator key.
//!
//! # Overview
//!
//! ProUpRevTx is one of the simplest provider transaction types in Dash.
//! It allows a masternode owner to revoke the operator's key, effectively
//! removing the operator's ability to participate in LLMQ and masternode
//! duties.
//!
//! # Transaction Type
//!
//! - **Transaction Type**: `TRANSACTION_PROVIDER_UPDATE_REVOKE` (8)
//! - **Versions**: 1 (Legacy BLS), 2 (Basic BLS)
//!
//! # Use Cases
//!
//! - Termination of service agreement with operator
//! - Operator key compromise
//! - Changing to a new operator key
//!
//! # Serialization Format
//!
//! ```text
//! version         (u16)      - Message version
//! pro_tx_hash     (32 bytes) - Hash of the initial ProRegTx
//! reason          (u16)      - Revocation reason (informational only)
//! inputs_hash     (32 bytes) - Hash of transaction inputs (replay protection)
//! sig             (96 bytes) - BLS signature from operator key
//! ```
//!
//! # Reference
//!
//! Dash Core implementation: `src/evo/providertx.h` (CProUpRevTx)

use crate::bls::BlsSignature;
use crate::error::{DashError, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Revocation reason codes
///
/// These are informational only and do not affect the revocation itself.
/// They provide context for why a masternode operator key was revoked.
///
/// # Note
///
/// The reason code does not impact the validity or processing of the
/// revocation transaction. It is purely for informational/logging purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RevocationReason {
    /// No specific reason provided (default)
    NotSpecified = 0,
    /// Service agreement was terminated
    TerminationOfService = 1,
    /// Operator's private key was compromised
    CompromisedKeys = 2,
    /// Changing to a new operator key
    ChangeOfKeys = 3,
}

impl TryFrom<u16> for RevocationReason {
    type Error = DashError;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            0 => Ok(RevocationReason::NotSpecified),
            1 => Ok(RevocationReason::TerminationOfService),
            2 => Ok(RevocationReason::CompromisedKeys),
            3 => Ok(RevocationReason::ChangeOfKeys),
            _ => Err(DashError::Serialization(format!(
                "Invalid revocation reason: {}",
                value
            ))),
        }
    }
}

/// Provider Update Revoke Transaction payload
///
/// Used to revoke a masternode's operator key. This is one of the simpler
/// provider transaction types in the Dash network.
///
/// # Fields
///
/// - `version`: Message version (1 = Legacy BLS, 2 = Basic BLS)
/// - `pro_tx_hash`: Hash of the original ProRegTx that registered this masternode
/// - `reason`: Informational reason code for the revocation
/// - `inputs_hash`: SHA256 hash of transaction inputs for replay protection
/// - `sig`: BLS signature from the operator key being revoked
///
/// # Example
///
/// ```
/// use librustdash::{ProUpRevTx, RevocationReason, BlsSignature};
///
/// let revoke_tx = ProUpRevTx {
///     version: 1,
///     pro_tx_hash: [0xAA; 32],
///     reason: RevocationReason::CompromisedKeys,
///     inputs_hash: [0xBB; 32],
///     sig: BlsSignature { data: vec![0xCC; 96] },
/// };
///
/// let bytes = revoke_tx.serialize().unwrap();
/// let decoded = ProUpRevTx::deserialize(&bytes).unwrap();
/// assert_eq!(revoke_tx, decoded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProUpRevTx {
    /// Message version (1 = Legacy BLS, 2 = Basic BLS)
    pub version: u16,
    /// Hash of the original ProRegTx
    pub pro_tx_hash: [u8; 32],
    /// Revocation reason (informational only)
    pub reason: RevocationReason,
    /// Hash of transaction inputs (replay protection)
    pub inputs_hash: [u8; 32],
    /// BLS signature from operator key
    pub sig: BlsSignature,
}

impl ProUpRevTx {
    /// Serialize the ProUpRevTx payload
    ///
    /// Serializes to bytes matching Dash Core's binary format.
    ///
    /// # Format
    ///
    /// ```text
    /// version         (u16)      - Little-endian
    /// pro_tx_hash     (32 bytes) - As-is
    /// reason          (u16)      - Little-endian
    /// inputs_hash     (32 bytes) - As-is
    /// sig             (96 bytes) - BLS signature
    /// ```
    ///
    /// Total size: 2 + 32 + 2 + 32 + 96 = 164 bytes
    ///
    /// # Errors
    ///
    /// Returns an error if the BLS signature is not exactly 96 bytes.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version
        buf.write_u16::<LittleEndian>(self.version)?;

        // Write pro_tx_hash
        buf.write_all(&self.pro_tx_hash)?;

        // Write reason
        buf.write_u16::<LittleEndian>(self.reason as u16)?;

        // Write inputs_hash
        buf.write_all(&self.inputs_hash)?;

        // Write BLS signature (validate size first)
        let sig_bytes = self.sig.serialize()?;
        buf.write_all(&sig_bytes)?;

        Ok(buf)
    }

    /// Deserialize a ProUpRevTx payload from bytes
    ///
    /// Reads the binary format produced by Dash Core.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Data is too short (< 164 bytes)
    /// - Revocation reason is invalid (> 3)
    /// - BLS signature is malformed
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version
        let version = cursor.read_u16::<LittleEndian>()?;

        // Read pro_tx_hash
        let mut pro_tx_hash = [0u8; 32];
        cursor.read_exact(&mut pro_tx_hash)?;

        // Read reason
        let reason_val = cursor.read_u16::<LittleEndian>()?;
        let reason = RevocationReason::try_from(reason_val)?;

        // Read inputs_hash
        let mut inputs_hash = [0u8; 32];
        cursor.read_exact(&mut inputs_hash)?;

        // Read BLS signature (96 bytes)
        let mut sig_data = vec![0u8; 96];
        cursor.read_exact(&mut sig_data)?;
        let sig = BlsSignature { data: sig_data };

        Ok(ProUpRevTx {
            version,
            pro_tx_hash,
            reason,
            inputs_hash,
            sig,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prouprevtx_v1_roundtrip() {
        let tx = ProUpRevTx {
            version: 1,
            pro_tx_hash: [0xAA; 32],
            reason: RevocationReason::NotSpecified,
            inputs_hash: [0xBB; 32],
            sig: BlsSignature {
                data: vec![0xCC; 96],
            },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpRevTx::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_prouprevtx_all_reasons() {
        for reason in [
            RevocationReason::NotSpecified,
            RevocationReason::TerminationOfService,
            RevocationReason::CompromisedKeys,
            RevocationReason::ChangeOfKeys,
        ] {
            let tx = ProUpRevTx {
                version: 1,
                pro_tx_hash: [0; 32],
                reason,
                inputs_hash: [0; 32],
                sig: BlsSignature { data: vec![0; 96] },
            };
            let bytes = tx.serialize().unwrap();
            let decoded = ProUpRevTx::deserialize(&bytes).unwrap();
            assert_eq!(tx.reason, decoded.reason);
        }
    }

    #[test]
    fn test_prouprevtx_invalid_signature() {
        let tx = ProUpRevTx {
            version: 1,
            pro_tx_hash: [0; 32],
            reason: RevocationReason::NotSpecified,
            inputs_hash: [0; 32],
            sig: BlsSignature {
                data: vec![0; 48], // Wrong size
            },
        };
        assert!(tx.serialize().is_err());
    }

    #[test]
    fn test_prouprevtx_different_versions() {
        for version in [1, 2] {
            let tx = ProUpRevTx {
                version,
                pro_tx_hash: [0x11; 32],
                reason: RevocationReason::CompromisedKeys,
                inputs_hash: [0x22; 32],
                sig: BlsSignature {
                    data: vec![0x33; 96],
                },
            };
            let bytes = tx.serialize().unwrap();
            let decoded = ProUpRevTx::deserialize(&bytes).unwrap();
            assert_eq!(tx.version, decoded.version);
        }
    }

    #[test]
    fn test_revocation_reason_values() {
        assert_eq!(RevocationReason::NotSpecified as u16, 0);
        assert_eq!(RevocationReason::TerminationOfService as u16, 1);
        assert_eq!(RevocationReason::CompromisedKeys as u16, 2);
        assert_eq!(RevocationReason::ChangeOfKeys as u16, 3);
    }
}
