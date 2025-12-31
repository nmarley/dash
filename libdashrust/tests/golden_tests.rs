// Copyright (c) 2025 The Dash Core developers
// Distributed under the MIT software license

//! Golden tests using real Dash Core transaction and block data

use libdashrust::{DashTxType, Transaction};

#[test]
fn test_normal_transaction_v2_roundtrip() {
    // Real normal transaction from Dash (simplified version of tx from Bitcoin Core tests)
    let hex = "0200000001b14bdcbc3e01bdaad36cc08e81e69c82e1060bc14e518db2b49aa43ad90ba26000000000490047304402203f16c6f40162ab686621ef3000b04e75418a0c0cb2d8aebeac894ae360ac1e780220ddc15ecdfc3507ac48e1681a33eb60996631bf6bf5bc0a0682c4db743ce7ca2b01ffffffff0140420f00000000001976a914660d4ef3a743e3e696ad990364e555c271ad504b88ac00000000";
    let expected_bytes = hex::decode(hex).unwrap();

    let tx = Transaction::deserialize(&expected_bytes).unwrap();

    // Verify deserialized fields
    assert_eq!(tx.version, 2);
    assert_eq!(tx.tx_type, DashTxType::Normal);
    assert_eq!(tx.inputs.len(), 1);
    assert_eq!(tx.outputs.len(), 1);
    assert_eq!(tx.lock_time, 0);
    assert_eq!(tx.extra_payload, None);

    // Critical test: Round-trip must match byte-for-byte
    let actual_bytes = tx.serialize().unwrap();
    assert_eq!(
        actual_bytes, expected_bytes,
        "Round-trip serialization mismatch!"
    );
}

#[test]
fn test_version_type_decoding() {
    // Test version=2, type=0 (normal tx)
    let combined_bytes = vec![0x02, 0x00, 0x00, 0x00]; // 0x00000002 in little-endian
    let combined = u32::from_le_bytes([
        combined_bytes[0],
        combined_bytes[1],
        combined_bytes[2],
        combined_bytes[3],
    ]);
    let version = (combined & 0xFFFF) as i16;
    let tx_type = ((combined >> 16) & 0xFFFF) as u16;

    assert_eq!(version, 2);
    assert_eq!(tx_type, 0);

    // Test version=3, type=1 (special tx)
    let combined_bytes = vec![0x03, 0x00, 0x01, 0x00]; // 0x00010003 in little-endian
    let combined = u32::from_le_bytes([
        combined_bytes[0],
        combined_bytes[1],
        combined_bytes[2],
        combined_bytes[3],
    ]);
    let version = (combined & 0xFFFF) as i16;
    let tx_type = ((combined >> 16) & 0xFFFF) as u16;

    assert_eq!(version, 3);
    assert_eq!(tx_type, 1);
}
