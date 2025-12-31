# Dash Serialization Format Reference

This document details Dash's binary serialization format, focusing on differences from Bitcoin. This is critical for librustdash implementation.

## Overview

Dash extends Bitcoin's serialization with:
- Combined version+type encoding in transactions
- Extra payload field for special transactions
- BLS signatures (48-96 bytes)
- Compact size varints
- Version-dependent field serialization

## Primitive Types

### Fixed-Size Types

```
Type        | Size    | Byte Order | Range
------------|---------|------------|------------------------
uint8_t     | 1 byte  | N/A        | 0 to 255
uint16_t    | 2 bytes | Little     | 0 to 65,535
uint32_t    | 4 bytes | Little     | 0 to 4,294,967,295
uint64_t    | 8 bytes | Little     | 0 to 2^64-1
int16_t     | 2 bytes | Little     | -32,768 to 32,767
int32_t     | 4 bytes | Little     | -2^31 to 2^31-1
int64_t     | 8 bytes | Little     | -2^63 to 2^63-1
```

### Variable-Size Types

**CompactSize (varint)**:
```
Value Range        | Format
-------------------|------------------------------------------
0 to 252           | 1 byte: value
253 to 65,535      | 3 bytes: 0xFD + 2-byte little-endian
65,536 to 2^32-1   | 5 bytes: 0xFE + 4-byte little-endian
2^32 to 2^64-1     | 9 bytes: 0xFF + 8-byte little-endian
```

**Example Rust implementation**:
```rust
fn write_compact_size<W: Write>(writer: &mut W, value: u64) -> Result<()> {
    match value {
        0..=252 => writer.write_u8(value as u8)?,
        253..=0xFFFF => {
            writer.write_u8(0xFD)?;
            writer.write_u16::<LittleEndian>(value as u16)?;
        }
        0x10000..=0xFFFFFFFF => {
            writer.write_u8(0xFE)?;
            writer.write_u32::<LittleEndian>(value as u32)?;
        }
        _ => {
            writer.write_u8(0xFF)?;
            writer.write_u64::<LittleEndian>(value)?;
        }
    }
    Ok(())
}
```

### Hash Types

**uint256 (32 bytes)**:
- Transaction IDs (txid)
- Block hashes
- Merkle roots
- Internal byte order (reversed for display)

**uint160 (20 bytes)**:
- Key IDs (RIPEMD-160 of public key hash)
- Platform node IDs

### Script and String Types

**Vector<T>**:
```
[CompactSize: length][T * length]
```

**Example**: `std::vector<unsigned char>`
```
Hex: 03 01 02 03
     ^^ --------
     |  3 bytes of data
     length = 3
```

## Block Serialization

### BlockHeader (80 bytes)

```
Offset | Size | Type    | Field
-------|------|---------|------------------
0      | 4    | int32   | nVersion
4      | 32   | uint256 | hashPrevBlock
36     | 32   | uint256 | hashMerkleRoot
68     | 4    | uint32  | nTime
72     | 4    | uint32  | nBits
76     | 4    | uint32  | nNonce
```

**Example (hex)**:
```
02000000  // nVersion = 2
7bc8...   // hashPrevBlock (32 bytes)
ab3e...   // hashMerkleRoot (32 bytes)
2a7c...   // nTime = 0x...
ffff001d  // nBits
1dac2b7c  // nNonce
```

### Block

```
[BlockHeader: 80 bytes]
[CompactSize: tx count]
[Transaction * tx count]
```

### Block File Format (.dat files)

```
┌─────────────────────────────────────┐
│  Magic Bytes (4 bytes)              │  Network-specific
├─────────────────────────────────────┤
│  Block Size (4 bytes, little-endian)│
├─────────────────────────────────────┤
│  Block Data (variable)              │
│    - Header (80 bytes)              │
│    - Tx count (varint)              │
│    - Transactions                   │
└─────────────────────────────────────┘
```

**Magic Bytes**:
- Mainnet: `0xBF0C6BBD`
- Testnet: `0xFFCAE2CE`
- Regtest: `0xFCC1B7DC`

**Example**:
```
BD 6B 0C BF      // Magic (mainnet)
E8 03 00 00      // Size = 1000 bytes
02 00 00 00      // Version
... (block data)
```

## Transaction Serialization

### Standard Bitcoin Transaction

```
[int32: version]
[CompactSize: input count]
[TxIn * input count]
[CompactSize: output count]
[TxOut * output count]
[uint32: lock_time]
```

### Dash Transaction (Extended)

**Key difference**: Version field is 32 bits combining version and type.

```
[int32: version_and_type]  // (type << 16) | version
[CompactSize: input count]
[TxIn * input count]
[CompactSize: output count]
[TxOut * output count]
[uint32: lock_time]
[optional: extra_payload]  // if version >= 3 && type != 0
```

**Version and Type Encoding**:
```rust
// Encoding
let combined = ((tx_type as u32) << 16) | (version as u32 & 0xFFFF);

// Decoding
let version = (combined & 0xFFFF) as i16;
let tx_type = (combined >> 16) as u16;
```

**Example**:
```
// Normal transaction (type=0, version=2)
02 00 00 00  → version=2, type=0

// Special transaction (type=1, version=3)
03 00 01 00  → version=3, type=1
             ↑↑ ↑↑ ↑↑ ↑↑
             v  v  t  t
             version=0x0003
             type=0x0001
```

### TxIn (Transaction Input)

```
[OutPoint: 36 bytes]
  [uint256: hash]     // Previous tx hash
  [uint32: index]     // Output index
[CompactSize + bytes: scriptSig]
[uint32: sequence]
```

**Example**:
```
// OutPoint
a1b2c3d4...  // 32-byte tx hash
02000000     // output index = 2

// scriptSig
19           // length = 25 bytes
76a914...    // 25 bytes of script

// sequence
ffffffff     // sequence = 0xFFFFFFFF
```

### TxOut (Transaction Output)

```
[int64: value]        // Amount in satoshis
[CompactSize + bytes: scriptPubKey]
```

**Example**:
```
// value
00f2052a01000000  // 5,000,000,000 satoshis (50 DASH)

// scriptPubKey
19                // length = 25 bytes
76a914...         // 25 bytes of script
```

## Special Transaction Payloads

### General Format

```
if (nVersion >= 3 && nType != TRANSACTION_NORMAL) {
    [CompactSize: payload_size]
    [bytes: payload_data]
}
```

### ProRegTx (Type 1: Provider Register)

```
[uint16: nVersion]            // ProTx version (1, 2, or 3)
[uint16: nType]               // Masternode type
[uint16: nMode]               // Mode (0 = normal)
[OutPoint: collateralOutpoint]
[NetInfo: network info]       // Depends on ProTx version
[uint160: platformNodeID]     // Only if version >= 2
[uint16: platformP2PPort]     // Only if version >= 2
[uint16: platformHTTPPort]    // Only if version >= 2
[uint160: keyIDOwner]
[BLSPubKey: pubKeyOperator]   // Size depends on legacy vs basic BLS
[uint160: keyIDVoting]
[uint16: nOperatorReward]
[Script: scriptPayout]
[uint256: inputsHash]
[CompactSize + bytes: vchSig]
```

**Version-dependent BLS key size**:
- Legacy BLS (v1): 48 bytes
- Basic BLS (v2+): 48 bytes (different scheme)

**Example structure**:
```
01 00        // version = 1 (LegacyBLS)
00 00        // type = 0 (Regular)
00 00        // mode = 0
... (outpoint)
... (network info)
... (rest of fields)
```

### CbTx (Type 5: Coinbase)

```
[uint16: version]
[int32: height]
[uint256: merkleRootMNList]
// if version >= 2:
[uint256: merkleRootQuorums]
// if version >= 3:
[CompactSize: bestCLHeightDiff]
[BLSSignature: bestCLSignature]
[int64: creditPoolBalance]
```

**Version evolution**:
- v1: MN list merkle root only
- v2: + Quorum merkle root
- v3: + ChainLock + credit pool balance

### QuorumCommitment (Type 6)

```
[uint16: version]
[uint8: llmqType]
[uint256: quorumHash]
// if version >= 2 or version >= 4 (indexed):
[int16: quorumIndex]
[CompactSize: signers_count]
[BitVector: signers]
[CompactSize: validMembers_count]
[BitVector: validMembers]
[BLSPubKey: quorumPublicKey]
[uint256: quorumVvecHash]
[BLSSignature: quorumSig]
[BLSSignature: membersSig]
```

**BitVector encoding**:
```
[CompactSize: byte_count]
[bytes: bits]
```

Example for 10 members:
```
02           // 2 bytes needed (10 bits)
A5 03        // Binary: 10100101 00000011
             // Members: [0,2,5,7,8,9] are set
```

### AssetLock (Type 8)

```
[uint8: version]
[CompactSize: creditOutputs_count]
[TxOut * creditOutputs_count]
```

### AssetUnlock (Type 9)

```
[uint8: version]
[uint64: index]
[int64: fee]
[uint32: height]
[uint256: quorumHash]
[BLSSignature: quorumSig]
```

## BLS Serialization

### BLS Public Key

**Legacy BLS (48 bytes)**:
```
[48 bytes: G1 point]
```

**Basic BLS (48 bytes)**:
```
[48 bytes: G1 point]  // Different serialization format
```

### BLS Signature (96 bytes)

```
[96 bytes: G2 point]
```

### Lazy BLS (Variable)

Some fields use "lazy" BLS where invalid keys are allowed:

```
if (key is valid) {
    [1 byte: 0x01]
    [48 bytes: key data]
} else {
    [1 byte: 0x00]
}
```

## Network Info Serialization

### Standard (IPv4/IPv6)

```
[CService: network address]
  [uint64: timestamp]     // Not serialized in ProTx
  [uint8: service flags]  // Not serialized in ProTx
  [16 bytes: IPv6 address]  // IPv4-mapped if IPv4
  [uint16: port]
```

### Extended Address (ProTx v3+)

```
[uint8: address type]
// if IPv4:
[4 bytes: IPv4 address]
// if IPv6:
[16 bytes: IPv6 address]
// if Tor:
[10 bytes: onion address]
// if I2P:
[32 bytes: I2P address]
[uint16: port]
```

## Versioned Serialization Patterns

Many Dash types use version-dependent serialization:

```rust
fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
    writer.write_u16(self.version)?;

    // Fields that exist in all versions
    writer.write_u32(self.common_field)?;

    // Conditional fields
    if self.version >= 2 {
        writer.write_u64(self.v2_field)?;
    }

    if self.version >= 3 {
        writer.write_bytes(&self.v3_field)?;
    }

    Ok(())
}
```

## Hashing

### Transaction Hash (txid)

```
txid = SHA256(SHA256(serialized_transaction))
```

**Important**: Serialize the transaction exactly as it appears on the network, including the extra payload.

### Block Hash

```
block_hash = SHA256(SHA256(block_header))
```

Only the 80-byte header is hashed, not the full block.

### Input Hash (for ProRegTx)

```
inputs_hash = SHA256(SHA256(serialized_inputs))
```

Used in ProRegTx for replay protection.

## Validation Rules (Serialization-Related)

### Version Checks

```rust
// Transaction version
if version < 1 || version > 3 {
    return Err("Invalid transaction version");
}

// Special tx requires version 3
if tx_type != NORMAL && version != 3 {
    return Err("Special tx must have version 3");
}
```

### Payload Size Limits

```
max_extra_payload_size = 10,000 bytes  // Consensus rule
```

### Compact Size Limits

```
// Vector size must fit in uint32
if count > 0xFFFFFFFF {
    return Err("Vector too large");
}
```

## Common Pitfalls

### 1. Byte Order

```
❌ WRONG: Writing 0x12345678 directly
✅ RIGHT: Write as little-endian: 78 56 34 12
```

### 2. Hash Display vs Storage

```
❌ WRONG: Storing displayed hash as-is
✅ RIGHT: Reverse bytes for storage/comparison

// Example txid display: "abc123..."
// Actual bytes: reverse of display
```

### 3. Version/Type Encoding

```
❌ WRONG: Writing version then type separately
✅ RIGHT: Combine first, then write as uint32

let combined = ((type as u32) << 16) | version as u32;
writer.write_u32(combined)?;
```

### 4. Optional Payload

```
❌ WRONG: Always writing extra_payload
✅ RIGHT: Check version and type first

if version >= 3 && tx_type != NORMAL {
    writer.write_payload(&extra_payload)?;
}
```

### 5. BLS Scheme Differences

```
❌ WRONG: Treating Legacy and Basic BLS identically
✅ RIGHT: Track which scheme is active

match protx.version {
    ProTxVersion::LegacyBLS => use_legacy_bls(),
    ProTxVersion::BasicBLS => use_basic_bls(),
    _ => unreachable!(),
}
```

## Testing Serialization

### Golden Test Approach

```rust
#[test]
fn test_transaction_serialization() {
    // Real transaction from mainnet
    let hex = "0300010001a1b2c3...";
    let expected = hex::decode(hex).unwrap();

    let tx = Transaction::deserialize(&expected).unwrap();
    let actual = tx.serialize().unwrap();

    assert_eq!(expected, actual, "Round-trip failed");
}
```

### Property Testing

```rust
proptest! {
    #[test]
    fn transaction_roundtrip(
        version in 1i16..4,
        tx_type in 0u16..10,
    ) {
        let tx = Transaction::new(version, tx_type, ...);
        let bytes = tx.serialize().unwrap();
        let decoded = Transaction::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }
}
```

## References

- [Bitcoin Serialization](https://en.bitcoin.it/wiki/Protocol_documentation#Variable_length_integer)
- [Dash Transaction Types](https://github.com/dashpay/dash/blob/develop/src/primitives/transaction.h)
- [Dash Special Transactions](https://github.com/dashpay/dash/tree/develop/src/evo)
- [BLS Signatures](https://github.com/dashpay/dips/blob/master/dip-0006.md)

## Appendix: Full Transaction Example

### Normal Transaction (Type 0, Version 2)

```
Hex breakdown:
02 00 00 00                    // version=2, type=0
01                             // 1 input
a1b2c3d4...                    // input 0: prev tx hash (32 bytes)
02 00 00 00                    // input 0: output index = 2
19                             // input 0: scriptSig length = 25
76a914...                      // input 0: scriptSig (25 bytes)
ff ff ff ff                    // input 0: sequence
02                             // 2 outputs
00 f2 05 2a 01 00 00 00        // output 0: value = 5,000,000,000
19                             // output 0: scriptPubKey length
76a914...                      // output 0: scriptPubKey (25 bytes)
00 e1 f5 05 00 00 00 00        // output 1: value = 100,000,000
19                             // output 1: scriptPubKey length
76a914...                      // output 1: scriptPubKey (25 bytes)
00 00 00 00                    // locktime = 0
                               // (no extra payload for type 0)
```

### Special Transaction (ProRegTx, Type 1, Version 3)

```
03 00 01 00                    // version=3, type=1
... (inputs/outputs same as above)
00 00 00 00                    // locktime

// Extra payload starts here
E5 00                          // payload size = 229 bytes
01 00                          // ProTx version = 1
00 00                          // mn type = 0
00 00                          // mode = 0
... (collateral outpoint)
... (network info)
... (all ProRegTx fields)
... (signature at end)
```
