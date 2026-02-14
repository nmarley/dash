//! Hash utilities for Dash blockchain
//!
//! Provides:
//! - SHA-256d (double SHA-256) for transaction IDs and merkle roots
//! - X11 block hashing (behind the `x11` feature flag)

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
    let second = Sha256::digest(first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Compute the X11 hash of data (first 32 bytes of the 64-byte X11 output).
///
/// This is used for Dash block hashing (Proof of Work). X11 chains 11
/// different hash algorithms: Blake, BMW, Groestl, Skein, JH, Keccak,
/// Luffa, CubeHash, ShaVite, SIMD, ECHO.
///
/// Returns the hash in internal byte order.
///
/// Requires the `x11` feature flag:
/// ```toml
/// librustdash = { version = "0.1", features = ["x11"] }
/// ```
#[cfg(feature = "x11")]
pub fn x11_hash(data: &[u8]) -> [u8; 32] {
    let full = x11_hash::hash(data);
    let mut result = [0u8; 32];
    result.copy_from_slice(&full[..32]);
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
        let hash = sha256d(b"");
        assert_eq!(
            hex::encode(hash),
            "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456"
        );
    }

    #[test]
    fn test_sha256d_known_value() {
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

    /// Test X11 block hash against the Dash genesis block.
    ///
    /// The genesis block header (80 bytes) should hash to:
    /// 00000ffd590b1485b3caadc19b22e6379c733355108f107a430458cdf3407ab6
    /// (display order)
    #[test]
    fn test_x11_genesis_block_hash() {
        // Dash genesis block header bytes (from the block file)
        // version=1, prev=0x00*32, merkle=e0028e..., time=1390095618, bits=0x1e0ffff0, nonce=28917698
        use crate::block::BlockHeader;

        let header = BlockHeader {
            version: 1,
            prev_blockhash: [0u8; 32],
            merkle_root: {
                // e0028eb9648db56b1ac77cf090b99048a8007e2bb64b68f092c03c7f56a662c7
                // in internal byte order (reversed from display)
                let mut mr = [0u8; 32];
                let display_hex =
                    "c762a6567f3cc092f0684bb62b7e00a84890b990f07cc71a6bb58d64b98e02e0";
                let bytes = hex::decode(display_hex).unwrap();
                mr.copy_from_slice(&bytes);
                mr
            },
            time: 1390095618,
            bits: 0x1e0ffff0,
            nonce: 28917698,
        };

        let header_bytes = header.serialize().unwrap();
        assert_eq!(header_bytes.len(), 80);

        // X11 hash it
        let hash = x11_hash::hash(&header_bytes);
        let block_hash = &hash[..32];

        // Convert to display order (reverse) and check against known genesis hash
        let mut hash32 = [0u8; 32];
        hash32.copy_from_slice(block_hash);
        let display = hash_to_display(&hash32);

        assert_eq!(
            display, "00000ffd590b1485b3caadc19b22e6379c733355108f107a430458cdf3407ab6",
            "X11 hash of genesis block header should match known genesis block hash"
        );
    }
}
