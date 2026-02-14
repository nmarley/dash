//! Script analysis utilities for Dash transactions.
//!
//! Provides classification and address extraction from scriptPubKey.
//! Dash uses the same script types as Bitcoin:
//! - P2PKH (Pay to Public Key Hash) -- the most common
//! - P2SH (Pay to Script Hash) -- used for multisig, etc.
//! - P2PK (Pay to Public Key) -- legacy, rare
//! - OP_RETURN (data carrier, unspendable)
//! - Nonstandard (everything else)

use crate::hash::sha256d;

/// The type of a scriptPubKey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptType {
    /// P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    P2pkh,
    /// P2SH: OP_HASH160 <20 bytes> OP_EQUAL
    P2sh,
    /// P2PK: <33 or 65 bytes> OP_CHECKSIG
    P2pk,
    /// OP_RETURN <data>
    OpReturn,
    /// Anything else
    Nonstandard,
}

/// Result of analyzing a scriptPubKey.
#[derive(Debug, Clone)]
pub struct ScriptInfo {
    /// Script type
    pub script_type: ScriptType,
    /// Extracted address hash (20 bytes for P2PKH/P2SH, None for others).
    /// For P2PK, this is the hash160 of the public key.
    pub address_hash: Option<[u8; 20]>,
}

/// Classify a scriptPubKey and extract the address hash if possible.
pub fn analyze_script(script: &[u8]) -> ScriptInfo {
    // P2PKH: 76 a9 14 <20 bytes> 88 ac (25 bytes total)
    if script.len() == 25
        && script[0] == 0x76  // OP_DUP
        && script[1] == 0xa9  // OP_HASH160
        && script[2] == 0x14  // push 20 bytes
        && script[23] == 0x88 // OP_EQUALVERIFY
        && script[24] == 0xac
    // OP_CHECKSIG
    {
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&script[3..23]);
        return ScriptInfo {
            script_type: ScriptType::P2pkh,
            address_hash: Some(hash),
        };
    }

    // P2SH: a9 14 <20 bytes> 87 (23 bytes total)
    if script.len() == 23
        && script[0] == 0xa9  // OP_HASH160
        && script[1] == 0x14  // push 20 bytes
        && script[22] == 0x87
    // OP_EQUAL
    {
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&script[2..22]);
        return ScriptInfo {
            script_type: ScriptType::P2sh,
            address_hash: Some(hash),
        };
    }

    // P2PK compressed: 21 <33 bytes> ac (35 bytes total)
    if script.len() == 35
        && script[0] == 0x21  // push 33 bytes
        && script[34] == 0xac // OP_CHECKSIG
        && (script[1] == 0x02 || script[1] == 0x03)
    // compressed pubkey prefix
    {
        let hash = hash160(&script[1..34]);
        return ScriptInfo {
            script_type: ScriptType::P2pk,
            address_hash: Some(hash),
        };
    }

    // P2PK uncompressed: 41 <65 bytes> ac (67 bytes total)
    if script.len() == 67
        && script[0] == 0x41  // push 65 bytes
        && script[66] == 0xac // OP_CHECKSIG
        && script[1] == 0x04
    // uncompressed pubkey prefix
    {
        let hash = hash160(&script[1..66]);
        return ScriptInfo {
            script_type: ScriptType::P2pk,
            address_hash: Some(hash),
        };
    }

    // OP_RETURN
    if !script.is_empty() && script[0] == 0x6a {
        return ScriptInfo {
            script_type: ScriptType::OpReturn,
            address_hash: None,
        };
    }

    ScriptInfo {
        script_type: ScriptType::Nonstandard,
        address_hash: None,
    }
}

/// Compute HASH160 (SHA256 then RIPEMD160) of data.
///
/// This is the standard address hash function in Bitcoin/Dash.
fn hash160(data: &[u8]) -> [u8; 20] {
    use bitcoin::hashes::{hash160, Hash};
    let h = hash160::Hash::hash(data);
    let mut result = [0u8; 20];
    result.copy_from_slice(h.as_ref());
    result
}

/// Encode an address hash as a Base58Check Dash address.
///
/// - P2PKH mainnet: version byte 0x4c (76) -> starts with 'X'
/// - P2SH mainnet: version byte 0x10 (16) -> starts with '7'
/// - P2PKH testnet: version byte 0x8c (140) -> starts with 'y'
/// - P2SH testnet: version byte 0x13 (19) -> starts with '8'
pub fn encode_address(hash: &[u8; 20], script_type: &ScriptType, testnet: bool) -> Option<String> {
    let version = match (script_type, testnet) {
        (ScriptType::P2pkh | ScriptType::P2pk, false) => 0x4c_u8,
        (ScriptType::P2sh, false) => 0x10_u8,
        (ScriptType::P2pkh | ScriptType::P2pk, true) => 0x8c_u8,
        (ScriptType::P2sh, true) => 0x13_u8,
        _ => return None,
    };

    // Build payload: version + hash
    let mut payload = Vec::with_capacity(25);
    payload.push(version);
    payload.extend_from_slice(hash);

    // Compute checksum: first 4 bytes of SHA256d(payload)
    let checksum = sha256d(&payload);
    payload.extend_from_slice(&checksum[..4]);

    Some(bs58_encode(&payload))
}

/// Simple Base58 encoding (Bitcoin alphabet).
fn bs58_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Count leading zeros
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // Convert to base58 using big-number division
    let mut num = data.to_vec();
    let mut result = Vec::new();

    while !num.is_empty() {
        let mut remainder = 0u32;
        let mut new_num = Vec::new();

        for &byte in &num {
            let acc = (remainder << 8) + byte as u32;
            let digit = acc / 58;
            remainder = acc % 58;

            if !new_num.is_empty() || digit > 0 {
                new_num.push(digit as u8);
            }
        }

        result.push(ALPHABET[remainder as usize]);
        num = new_num;
    }

    // Add leading '1's for leading zero bytes
    for _ in 0..leading_zeros {
        result.push(b'1');
    }

    result.reverse();
    String::from_utf8(result).unwrap()
}

/// Decode a Base58Check Dash address into (version_byte, 20-byte hash).
///
/// Returns None if the address is invalid (bad checksum, wrong length, etc.).
pub fn decode_address(addr: &str) -> Option<(u8, [u8; 20])> {
    let decoded = bs58_decode(addr)?;
    // Must be exactly 25 bytes: 1 version + 20 hash + 4 checksum
    if decoded.len() != 25 {
        return None;
    }

    // Verify checksum
    let payload = &decoded[..21];
    let expected_checksum = &decoded[21..25];
    let actual_checksum = sha256d(payload);
    if expected_checksum != &actual_checksum[..4] {
        return None;
    }

    let version = decoded[0];
    let mut hash = [0u8; 20];
    hash.copy_from_slice(&decoded[1..21]);
    Some((version, hash))
}

/// Determine if the version byte corresponds to a known Dash address type.
///
/// Returns the ScriptType that this address version corresponds to, or None
/// if the version byte is unknown.
pub fn script_type_from_version(version: u8) -> Option<ScriptType> {
    match version {
        0x4c | 0x8c => Some(ScriptType::P2pkh), // mainnet / testnet P2PKH
        0x10 | 0x13 => Some(ScriptType::P2sh),  // mainnet / testnet P2SH
        _ => None,
    }
}

/// Simple Base58 decoding (Bitcoin alphabet).
fn bs58_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Count leading '1' chars (leading zero bytes)
    let leading_ones = input.bytes().take_while(|&b| b == b'1').count();

    // Convert from base58
    let mut num: Vec<u8> = Vec::new();
    for ch in input.bytes() {
        let val = ALPHABET.iter().position(|&c| c == ch)? as u32;

        // Multiply num by 58 and add val
        let mut carry = val;
        for byte in num.iter_mut().rev() {
            let acc = (*byte as u32) * 58 + carry;
            *byte = (acc & 0xff) as u8;
            carry = acc >> 8;
        }
        while carry > 0 {
            num.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    // Prepend leading zeros
    let mut result = vec![0u8; leading_ones];
    result.extend_from_slice(&num);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2pkh_script() {
        // Standard P2PKH script
        let script = hex::decode("76a91489abcdefabbaabbaabbaabbaabbaabbaabbaabba88ac").unwrap();
        let info = analyze_script(&script);
        assert_eq!(info.script_type, ScriptType::P2pkh);
        assert!(info.address_hash.is_some());
        assert_eq!(
            hex::encode(info.address_hash.unwrap()),
            "89abcdefabbaabbaabbaabbaabbaabbaabbaabba"
        );
    }

    #[test]
    fn test_p2sh_script() {
        // Standard P2SH script
        let script = hex::decode("a91489abcdefabbaabbaabbaabbaabbaabbaabbaabba87").unwrap();
        let info = analyze_script(&script);
        assert_eq!(info.script_type, ScriptType::P2sh);
        assert!(info.address_hash.is_some());
    }

    #[test]
    fn test_p2pk_compressed_script() {
        // P2PK with compressed pubkey (02 prefix)
        let mut script = vec![0x21]; // push 33 bytes
        script.push(0x02); // compressed prefix
        script.extend_from_slice(&[0xAA; 32]); // pubkey x-coord
        script.push(0xac); // OP_CHECKSIG
        assert_eq!(script.len(), 35);

        let info = analyze_script(&script);
        assert_eq!(info.script_type, ScriptType::P2pk);
        assert!(info.address_hash.is_some());
    }

    #[test]
    fn test_op_return_script() {
        let script = hex::decode("6a0b68656c6c6f20776f726c64").unwrap();
        let info = analyze_script(&script);
        assert_eq!(info.script_type, ScriptType::OpReturn);
        assert!(info.address_hash.is_none());
    }

    #[test]
    fn test_nonstandard_script() {
        let script = vec![0x51]; // OP_1 (not a standard output)
        let info = analyze_script(&script);
        assert_eq!(info.script_type, ScriptType::Nonstandard);
        assert!(info.address_hash.is_none());
    }

    #[test]
    fn test_encode_p2pkh_mainnet() {
        // Known Dash address: the genesis coinbase output
        // We test that the encoding produces a valid-looking Dash address
        let hash = [0u8; 20]; // dummy
        let addr = encode_address(&hash, &ScriptType::P2pkh, false).unwrap();
        // Dash mainnet P2PKH addresses start with 'X'
        assert!(addr.starts_with('X'), "Got: {}", addr);
    }

    #[test]
    fn test_encode_p2sh_mainnet() {
        let hash = [0u8; 20];
        let addr = encode_address(&hash, &ScriptType::P2sh, false).unwrap();
        // Dash mainnet P2SH addresses start with '7'
        assert!(addr.starts_with('7'), "Got: {}", addr);
    }

    #[test]
    fn test_encode_p2pkh_testnet() {
        let hash = [0u8; 20];
        let addr = encode_address(&hash, &ScriptType::P2pkh, true).unwrap();
        // Dash testnet P2PKH addresses start with 'y'
        assert!(addr.starts_with('y'), "Got: {}", addr);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original_hash = [0x42u8; 20];

        // P2PKH mainnet
        let addr = encode_address(&original_hash, &ScriptType::P2pkh, false).unwrap();
        let (version, decoded_hash) = decode_address(&addr).unwrap();
        assert_eq!(version, 0x4c);
        assert_eq!(decoded_hash, original_hash);

        // P2SH mainnet
        let addr = encode_address(&original_hash, &ScriptType::P2sh, false).unwrap();
        let (version, decoded_hash) = decode_address(&addr).unwrap();
        assert_eq!(version, 0x10);
        assert_eq!(decoded_hash, original_hash);

        // P2PKH testnet
        let addr = encode_address(&original_hash, &ScriptType::P2pkh, true).unwrap();
        let (version, decoded_hash) = decode_address(&addr).unwrap();
        assert_eq!(version, 0x8c);
        assert_eq!(decoded_hash, original_hash);
    }

    #[test]
    fn test_decode_invalid_address() {
        // Too short
        assert!(decode_address("1").is_none());
        // Invalid base58 character
        assert!(decode_address("X0OOO").is_none());
        // Random garbage
        assert!(decode_address("notanaddress").is_none());
    }
}
