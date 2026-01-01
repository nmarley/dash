//! BLS cryptographic key and signature types
//!
//! This module provides wrapper types for BLS (Boneh-Lynn-Shacham) public keys
//! and signatures used in Dash's masternode and LLMQ systems.
//!
//! Note: These are currently opaque byte wrappers. Actual BLS verification
//! will be added in a future phase.

use crate::error::{DashError, Result};
use std::io::Read;

/// BLS public key (48 bytes)
///
/// Used for masternode operator keys and quorum public keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlsPublicKey {
    pub data: Vec<u8>,
}

impl BlsPublicKey {
    /// Create a new BLS public key from bytes
    ///
    /// # Errors
    /// Returns `InvalidBlsPublicKey` if data is not exactly 48 bytes
    pub fn new(data: Vec<u8>) -> Result<Self> {
        if data.len() != 48 {
            return Err(DashError::InvalidBlsPublicKey);
        }
        Ok(BlsPublicKey { data })
    }

    /// Serialize the BLS public key
    pub fn serialize(&self) -> Result<Vec<u8>> {
        if self.data.len() != 48 {
            return Err(DashError::InvalidBlsPublicKey);
        }
        Ok(self.data.clone())
    }

    /// Deserialize a BLS public key from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 48 {
            return Err(DashError::InvalidBlsPublicKey);
        }
        let mut key_data = vec![0u8; 48];
        let mut cursor = std::io::Cursor::new(data);
        cursor.read_exact(&mut key_data)?;
        Ok(BlsPublicKey { data: key_data })
    }
}

/// BLS signature (96 bytes)
///
/// Used for masternode signatures, quorum signatures, and chainlock signatures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlsSignature {
    pub data: Vec<u8>,
}

impl BlsSignature {
    /// Create a new BLS signature from bytes
    ///
    /// # Errors
    /// Returns `InvalidBlsSignature` if data is not exactly 96 bytes
    pub fn new(data: Vec<u8>) -> Result<Self> {
        if data.len() != 96 {
            return Err(DashError::InvalidBlsSignature);
        }
        Ok(BlsSignature { data })
    }

    /// Serialize the BLS signature
    pub fn serialize(&self) -> Result<Vec<u8>> {
        if self.data.len() != 96 {
            return Err(DashError::InvalidBlsSignature);
        }
        Ok(self.data.clone())
    }

    /// Deserialize a BLS signature from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 96 {
            return Err(DashError::InvalidBlsSignature);
        }
        let mut sig_data = vec![0u8; 96];
        let mut cursor = std::io::Cursor::new(data);
        cursor.read_exact(&mut sig_data)?;
        Ok(BlsSignature { data: sig_data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bls_public_key_serialize() {
        let key = BlsPublicKey {
            data: vec![0xAA; 48],
        };
        let bytes = key.serialize().unwrap();
        assert_eq!(bytes.len(), 48);
        assert_eq!(bytes, vec![0xAA; 48]);
    }

    #[test]
    fn test_bls_public_key_deserialize() {
        let data = vec![0xBB; 48];
        let key = BlsPublicKey::deserialize(&data).unwrap();
        assert_eq!(key.data.len(), 48);
        assert_eq!(key.data, vec![0xBB; 48]);
    }

    #[test]
    fn test_bls_public_key_roundtrip() {
        let original = BlsPublicKey {
            data: vec![0xCC; 48],
        };
        let bytes = original.serialize().unwrap();
        let decoded = BlsPublicKey::deserialize(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_bls_public_key_invalid_size_serialize() {
        let bad_key = BlsPublicKey {
            data: vec![0; 47], // Wrong size
        };
        assert!(bad_key.serialize().is_err());

        let bad_key2 = BlsPublicKey {
            data: vec![0; 49], // Wrong size
        };
        assert!(bad_key2.serialize().is_err());
    }

    #[test]
    fn test_bls_public_key_invalid_size_deserialize() {
        let bad_data = vec![0; 47]; // Too short
        assert!(BlsPublicKey::deserialize(&bad_data).is_err());
    }

    #[test]
    fn test_bls_public_key_new() {
        let key = BlsPublicKey::new(vec![0; 48]).unwrap();
        assert_eq!(key.data.len(), 48);

        let bad_key = BlsPublicKey::new(vec![0; 47]);
        assert!(bad_key.is_err());
    }

    #[test]
    fn test_bls_signature_serialize() {
        let sig = BlsSignature {
            data: vec![0xBB; 96],
        };
        let bytes = sig.serialize().unwrap();
        assert_eq!(bytes.len(), 96);
        assert_eq!(bytes, vec![0xBB; 96]);
    }

    #[test]
    fn test_bls_signature_deserialize() {
        let data = vec![0xCC; 96];
        let sig = BlsSignature::deserialize(&data).unwrap();
        assert_eq!(sig.data.len(), 96);
        assert_eq!(sig.data, vec![0xCC; 96]);
    }

    #[test]
    fn test_bls_signature_roundtrip() {
        let original = BlsSignature {
            data: vec![0xDD; 96],
        };
        let bytes = original.serialize().unwrap();
        let decoded = BlsSignature::deserialize(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_bls_signature_invalid_size_serialize() {
        let bad_sig = BlsSignature {
            data: vec![0; 95], // Wrong size
        };
        assert!(bad_sig.serialize().is_err());

        let bad_sig2 = BlsSignature {
            data: vec![0; 97], // Wrong size
        };
        assert!(bad_sig2.serialize().is_err());
    }

    #[test]
    fn test_bls_signature_invalid_size_deserialize() {
        let bad_data = vec![0; 95]; // Too short
        assert!(BlsSignature::deserialize(&bad_data).is_err());
    }

    #[test]
    fn test_bls_signature_new() {
        let sig = BlsSignature::new(vec![0; 96]).unwrap();
        assert_eq!(sig.data.len(), 96);

        let bad_sig = BlsSignature::new(vec![0; 95]);
        assert!(bad_sig.is_err());
    }

    #[test]
    fn test_bls_different_values() {
        let key1 = BlsPublicKey {
            data: vec![0x11; 48],
        };
        let key2 = BlsPublicKey {
            data: vec![0x22; 48],
        };
        assert_ne!(key1, key2);

        let sig1 = BlsSignature {
            data: vec![0x33; 96],
        };
        let sig2 = BlsSignature {
            data: vec![0x44; 96],
        };
        assert_ne!(sig1, sig2);
    }
}
