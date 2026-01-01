//! Masternode Hard Fork Signal (MnhfSignal)
//!
//! This module implements the MnhfSignal transaction payload type, used for coordinating
//! hard fork activations via LLMQ quorum voting. The MnhfSignal transaction is part of the
//! Enhanced Hard Fork (EHF) mechanism introduced in Dash Core v19.
//!
//! The EHF mechanism allows masternodes to signal their readiness for network upgrades
//! through LLMQ quorums, providing a more decentralized and secure activation process
//! compared to traditional miner-activated soft forks.
//!
//! # Structure
//!
//! - `MnhfTx`: The core signal containing a BIP9 version bit, LLMQ quorum hash, and BLS signature
//! - `MnhfTxPayload`: Version wrapper for the signal
//!
//! # Serialization Format
//!
//! ```text
//! MnhfTx:
//! - version_bit: u8 (1 byte)
//! - quorum_hash: [u8; 32] (32 bytes)
//! - sig: BlsSignature (96 bytes)
//! Total: 129 bytes
//!
//! MnhfTxPayload:
//! - version: u8 (1 byte)
//! - signal: MnhfTx (129 bytes)
//! Total: 130 bytes
//! ```
//!
//! # Example
//!
//! ```
//! use librustdash::payloads::{MnhfTx, MnhfTxPayload};
//! use librustdash::bls::BlsSignature;
//!
//! // Create a hard fork signal for BIP9 bit 5
//! let signal = MnhfTx {
//!     version_bit: 5,
//!     quorum_hash: [0; 32],
//!     sig: BlsSignature {
//!         data: vec![0; 96],
//!     },
//! };
//!
//! let payload = MnhfTxPayload {
//!     version: 1,
//!     signal,
//! };
//!
//! let serialized = payload.serialize().unwrap();
//! assert_eq!(serialized.len(), 130);
//! ```
//!
//! # References
//!
//! - Dash Core implementation: `src/evo/mnhftx.h`
//! - DIP-0023: Enhanced Hard Fork Mechanism

use crate::bls::BlsSignature;
use crate::error::{DashError, Result};
use std::io::{Read, Write};

/// Masternode Hard Fork Transaction
///
/// Signals support for a specific hard fork using a BIP9 version bit. This transaction
/// is signed by an LLMQ quorum to indicate collective support for a network upgrade.
///
/// # Fields
///
/// - `version_bit`: BIP9 version bit (0-31) identifying the hard fork
/// - `quorum_hash`: Hash of the LLMQ quorum that signed this signal
/// - `sig`: BLS signature (96 bytes) from the quorum using Basic BLS scheme
///
/// # Serialization
///
/// Total size: 129 bytes (1 + 32 + 96)
///
/// ```text
/// [version_bit: 1 byte][quorum_hash: 32 bytes][sig: 96 bytes]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MnhfTx {
    /// BIP9 version bit (0-31) for the hard fork deployment
    pub version_bit: u8,

    /// LLMQ quorum hash (32 bytes)
    pub quorum_hash: [u8; 32],

    /// BLS signature from the quorum (96 bytes, Basic scheme)
    pub sig: BlsSignature,
}

impl MnhfTx {
    /// Serialize the MnhfTx to bytes
    ///
    /// # Returns
    ///
    /// A 129-byte vector containing the serialized transaction.
    ///
    /// # Errors
    ///
    /// Returns `DashError::InvalidBlsSignature` if the signature is not exactly 96 bytes.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version_bit (u8)
        buf.write_all(&[self.version_bit])?;

        // Write quorum_hash (32 bytes)
        buf.write_all(&self.quorum_hash)?;

        // Validate and write BLS signature (96 bytes)
        if self.sig.data.len() != 96 {
            return Err(DashError::InvalidBlsSignature);
        }
        buf.write_all(&self.sig.data)?;

        Ok(buf)
    }

    /// Deserialize a MnhfTx from bytes
    ///
    /// # Arguments
    ///
    /// - `data`: Byte slice containing the serialized transaction (minimum 129 bytes)
    ///
    /// # Errors
    ///
    /// Returns `DashError::Io` if there are insufficient bytes to read.
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version_bit (u8)
        let mut version_bit_buf = [0u8; 1];
        cursor.read_exact(&mut version_bit_buf)?;
        let version_bit = version_bit_buf[0];

        // Read quorum_hash (32 bytes)
        let mut quorum_hash = [0u8; 32];
        cursor.read_exact(&mut quorum_hash)?;

        // Read BLS signature (96 bytes)
        let mut sig_data = vec![0u8; 96];
        cursor.read_exact(&mut sig_data)?;
        let sig = BlsSignature { data: sig_data };

        Ok(MnhfTx {
            version_bit,
            quorum_hash,
            sig,
        })
    }
}

/// Masternode Hard Fork Transaction Payload
///
/// Version wrapper for the MnhfTx signal. This is the top-level payload structure
/// embedded in special transactions of type TRANSACTION_MNHF_SIGNAL.
///
/// # Fields
///
/// - `version`: Payload version (currently 1)
/// - `signal`: The actual hard fork signal
///
/// # Serialization
///
/// Total size: 130 bytes (1 + 129)
///
/// ```text
/// [version: 1 byte][signal: 129 bytes]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MnhfTxPayload {
    /// Payload version
    pub version: u8,

    /// The hard fork signal
    pub signal: MnhfTx,
}

impl MnhfTxPayload {
    /// Serialize the payload to bytes
    ///
    /// # Returns
    ///
    /// A 130-byte vector containing the serialized payload.
    ///
    /// # Errors
    ///
    /// Returns `DashError::InvalidBlsSignature` if the signal's signature is not exactly 96 bytes.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version (u8)
        buf.write_all(&[self.version])?;

        // Write signal (MnhfTx)
        let signal_bytes = self.signal.serialize()?;
        buf.write_all(&signal_bytes)?;

        Ok(buf)
    }

    /// Deserialize a payload from bytes
    ///
    /// # Arguments
    ///
    /// - `data`: Byte slice containing the serialized payload (minimum 130 bytes)
    ///
    /// # Errors
    ///
    /// Returns `DashError::Io` if there are insufficient bytes to read.
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version (u8)
        let mut version_buf = [0u8; 1];
        cursor.read_exact(&mut version_buf)?;
        let version = version_buf[0];

        // Read remaining bytes for signal
        let position = cursor.position() as usize;
        let signal = MnhfTx::deserialize(&data[position..])?;

        Ok(MnhfTxPayload { version, signal })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnhf_tx_roundtrip() {
        let tx = MnhfTx {
            version_bit: 5,
            quorum_hash: [0xAA; 32],
            sig: BlsSignature {
                data: vec![0xBB; 96],
            },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = MnhfTx::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_mnhf_payload_roundtrip() {
        let payload = MnhfTxPayload {
            version: 1,
            signal: MnhfTx {
                version_bit: 3,
                quorum_hash: [0; 32],
                sig: BlsSignature { data: vec![0; 96] },
            },
        };

        let bytes = payload.serialize().unwrap();
        let decoded = MnhfTxPayload::deserialize(&bytes).unwrap();
        assert_eq!(payload.version, 1);
        assert_eq!(payload.signal.version_bit, 3);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_mnhf_version_bit_range() {
        // Version bits should be 0-31 for BIP9 deployment bits
        // Though serialization doesn't enforce this (validation is at consensus level)
        for bit in [0, 1, 15, 28, 31] {
            let tx = MnhfTx {
                version_bit: bit,
                quorum_hash: [0; 32],
                sig: BlsSignature { data: vec![0; 96] },
            };
            let bytes = tx.serialize().unwrap();
            let decoded = MnhfTx::deserialize(&bytes).unwrap();
            assert_eq!(decoded.version_bit, bit);
        }
    }

    #[test]
    fn test_mnhf_invalid_signature() {
        let tx = MnhfTx {
            version_bit: 5,
            quorum_hash: [0; 32],
            sig: BlsSignature {
                data: vec![0; 48], // Wrong size
            },
        };
        assert!(tx.serialize().is_err());
    }

    #[test]
    fn test_mnhf_different_quorum_hashes() {
        let tx1 = MnhfTx {
            version_bit: 5,
            quorum_hash: [0x11; 32],
            sig: BlsSignature {
                data: vec![0x22; 96],
            },
        };
        let tx2 = MnhfTx {
            version_bit: 5,
            quorum_hash: [0x33; 32],
            sig: BlsSignature {
                data: vec![0x22; 96],
            },
        };

        assert_ne!(tx1, tx2);
        let bytes1 = tx1.serialize().unwrap();
        let bytes2 = tx2.serialize().unwrap();
        assert_ne!(bytes1, bytes2);
    }
}
