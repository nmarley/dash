//! Hash utilities for Dash blockchain
//!
//! Provides SHA-256d (double SHA-256) hashing used for transaction IDs
//! and merkle root computation. Block hashes in Dash use X11, which is
//! not implemented here -- use dashd RPC or the block index for those.

use sha2::{Digest, Sha256};

/// Compute SHA-256d (double SHA-256) of data.
///
/// This is the standard hash function used in Bitcoin/Dash for:
/// - Transaction IDs (txid)
/// - Merkle root computation
/// - Various internal hashes
///
/// Returns the hash in internal byte order (NOT display/RPC order).
/// For display order, reverse the bytes.
pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(&first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Reverse a 32-byte hash for display purposes.
///
/// Bitcoin/Dash hashes are displayed in reverse byte order compared to
/// their internal representation. This function converts between the two.
pub fn reverse_hash(hash: &[u8; 32]) -> [u8; 32] {
    let mut reversed = *hash;
    reversed.reverse();
    reversed
}

/// Format a 32-byte hash in display order (reversed, hex-encoded).
///
/// This produces the same format as `getblock`, `getrawtransaction`, etc.
pub fn hash_to_display(hash: &[u8; 32]) -> String {
    hex::encode(reverse_hash(hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256d_empty() {
        // SHA256d of empty data is a well-known value
        // SHA256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        // SHA256(above) = 5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456
        let hash = sha256d(b"");
        assert_eq!(
            hex::encode(hash),
            "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456"
        );
    }

    #[test]
    fn test_sha256d_known_value() {
        // "hello" -> SHA256d
        // SHA256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        // SHA256(above) = 9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50
        let hash = sha256d(b"hello");
        assert_eq!(
            hex::encode(hash),
            "9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50"
        );
    }

    #[test]
    fn test_reverse_hash() {
        let hash = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20,
        ];
        let reversed = reverse_hash(&hash);
        assert_eq!(reversed[0], 0x20);
        assert_eq!(reversed[31], 0x01);
    }

    #[test]
    fn test_hash_to_display() {
        let mut hash = [0u8; 32];
        hash[0] = 0xab;
        hash[31] = 0xcd;
        let display = hash_to_display(&hash);
        assert!(display.starts_with("cd"));
        assert!(display.ends_with("ab"));
    }
}
