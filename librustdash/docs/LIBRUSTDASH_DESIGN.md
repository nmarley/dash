# librustdash Design Document

## Vision

Build a foundational Rust library for Dash blockchain primitives, following the `librustzcash` pattern. This library provides reusable building blocks for any Rust-based Dash tooling: indexers, wallets, explorers, analytics, mobile SDKs, and future core components.

## Goals

1. **Reusable Primitives**: Core types that any Dash Rust project can depend on
2. **Correct Serialization**: Bug-for-bug compatible with Dash Core's binary format
3. **Type Safety**: Leverage Rust's type system to prevent invalid states
4. **Well Tested**: Comprehensive test suite against Dash Core outputs
5. **Foundation for Modernization**: Enable gradual migration of Dash Core components to Rust

## Non-Goals (Phase 1)

- Consensus validation logic (no script execution, no block validation)
- Wallet functionality (no key management, no signing)
- Network protocol (no P2P networking)
- RPC client/server
- LLMQ cryptographic operations (just parse the data structures)
- Replace Dash Core (this is a long-term goal, not Phase 1)

## Scope: What Goes in librustdash?

### Phase 1: Primitives Only

**YES - Include:**
- Block and transaction data structures
- Serialization/deserialization (binary format)
- Special transaction type enums and payloads
- Hashing functions (block hash, tx hash)
- Basic type conversions

**NO - Exclude (for now):**
- Script execution
- Signature verification (BLS, ECDSA)
- UTXO set management
- Mempool logic
- Consensus rules
- Network messages
- Database layer

### Library Structure

```
librustdash/
├── dash-primitives/          # Core types (Phase 1)
│   ├── block.rs              # Block, BlockHeader
│   ├── transaction.rs        # Transaction, TxIn, TxOut
│   ├── special_tx.rs         # DashTxType enum
│   ├── payloads/             # Special tx payloads
│   │   ├── mod.rs
│   │   ├── provider.rs       # ProRegTx, ProUpServTx, etc.
│   │   ├── coinbase.rs       # CCbTx
│   │   ├── quorum.rs         # CFinalCommitment
│   │   ├── asset_lock.rs     # CAssetLock/Unlock
│   │   └── mnhf.rs           # MNHFTxPayload
│   ├── hash.rs               # Hashing utilities
│   ├── serialize.rs          # Serialization traits
│   └── types.rs              # Common types (uint160, etc.)
│
├── dash-consensus/           # Consensus rules (Phase 2+)
├── dash-script/              # Script execution (Phase 2+)
├── dash-bls/                 # BLS operations (Phase 2+)
└── examples/                 # Usage examples
    ├── parse_block.rs
    ├── decode_tx.rs
    └── special_tx_types.rs
```

## Design Principles

### 1. Use Existing Bitcoin Crates Where Possible

**Leverage `bitcoin` crate:**
- `bitcoin::BlockHash`, `bitcoin::Txid` - Proven hash types
- `bitcoin::Script` - Script representation
- `bitcoin::Amount` - Satoshi amounts
- `bitcoin::OutPoint` - Transaction output reference

**Extend where Dash differs:**
- Special transaction types
- Version+Type encoding
- Extra payloads

### 2. Type-Safe Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DashTxType {
    Normal = 0,
    ProviderRegister = 1,
    ProviderUpdateService = 2,
    ProviderUpdateRegistrar = 3,
    ProviderUpdateRevoke = 4,
    Coinbase = 5,
    QuorumCommitment = 6,
    MnhfSignal = 7,
    AssetLock = 8,
    AssetUnlock = 9,
}
```

### 3. Versioned Payloads

Each special tx payload has its own versioning:

```rust
pub struct ProRegTx {
    pub version: ProTxVersion,
    pub mn_type: MasternodeType,
    pub mode: u16,
    pub collateral_outpoint: OutPoint,
    pub network_info: NetworkInfo,
    // ... version-specific fields
}

pub enum ProTxVersion {
    LegacyBLS = 1,
    BasicBLS = 2,
    ExtAddr = 3,
}
```

### 4. Serialization Format Compatibility

**Critical**: Must match Dash Core byte-for-byte.

**Dash's version+type encoding:**
```
n32bitVersion = (nType << 16) | nVersion
```

**Rust approach:**
```rust
impl Transaction {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let combined = ((self.tx_type as u32) << 16) | (self.version as u32);
        writer.write_u32::<LittleEndian>(combined)?;
        // ... rest of serialization
    }
}
```

### 5. Error Handling

```rust
#[derive(Debug, Error)]
pub enum DashError {
    #[error("Invalid transaction version: {0}")]
    InvalidVersion(u16),

    #[error("Unknown transaction type: {0}")]
    UnknownTxType(u16),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid payload for tx type {tx_type:?}")]
    InvalidPayload { tx_type: DashTxType },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

## Detailed Type Mappings

### Block Structure

**Dash Core (C++):**
```cpp
class CBlockHeader {
    int32_t nVersion;
    uint256 hashPrevBlock;
    uint256 hashMerkleRoot;
    uint32_t nTime;
    uint32_t nBits;
    uint32_t nNonce;
};

class CBlock : public CBlockHeader {
    std::vector<CTransactionRef> vtx;
};
```

**librustdash (Rust):**
```rust
use bitcoin::{BlockHash, TxMerkleNode};

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub version: i32,
    pub prev_blockhash: BlockHash,
    pub merkle_root: TxMerkleNode,
    pub time: u32,
    pub bits: u32,
    pub nonce: u32,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}
```

### Transaction Structure

**Dash Core (C++):**
```cpp
class CTransaction {
    const std::vector<CTxIn> vin;
    const std::vector<CTxOut> vout;
    const int16_t nVersion;
    const uint16_t nType;
    const uint32_t nLockTime;
    const std::vector<uint8_t> vExtraPayload;
};
```

**librustdash (Rust):**
```rust
use bitcoin::{OutPoint, Script, Amount, Txid};

#[derive(Debug, Clone)]
pub struct Transaction {
    pub version: i16,
    pub tx_type: DashTxType,
    pub inputs: Vec<TxIn>,
    pub outputs: Vec<TxOut>,
    pub lock_time: u32,
    pub extra_payload: Option<ExtraPayload>,
}

#[derive(Debug, Clone)]
pub struct TxIn {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

#[derive(Debug, Clone)]
pub struct TxOut {
    pub value: Amount,
    pub script_pubkey: Script,
}
```

### Special Transaction Payloads

**Enum for all payload types:**
```rust
#[derive(Debug, Clone)]
pub enum ExtraPayload {
    ProviderRegister(ProRegTx),
    ProviderUpdateService(ProUpServTx),
    ProviderUpdateRegistrar(ProUpRegTx),
    ProviderUpdateRevoke(ProUpRevTx),
    Coinbase(CbTx),
    QuorumCommitment(FinalCommitment),
    MnhfSignal(MnhfTx),
    AssetLock(AssetLockTx),
    AssetUnlock(AssetUnlockTx),
}

impl ExtraPayload {
    pub fn from_bytes(tx_type: DashTxType, data: &[u8]) -> Result<Self> {
        match tx_type {
            DashTxType::ProviderRegister => {
                Ok(ExtraPayload::ProviderRegister(
                    ProRegTx::deserialize(data)?
                ))
            }
            // ... other types
        }
    }
}
```

### Provider Transaction (Masternode)

**Key payload (simplified):**
```rust
#[derive(Debug, Clone)]
pub struct ProRegTx {
    pub version: ProTxVersion,
    pub mn_type: MasternodeType,
    pub mode: u16,
    pub collateral_outpoint: OutPoint,
    pub network_info: NetworkInfo,
    pub platform_node_id: [u8; 20], // uint160
    pub platform_p2p_port: u16,
    pub platform_http_port: u16,
    pub key_id_owner: [u8; 20],
    pub pubkey_operator: BlsPublicKey, // Opaque for Phase 1
    pub key_id_voting: [u8; 20],
    pub operator_reward: u16,
    pub script_payout: Script,
    pub inputs_hash: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProTxVersion {
    LegacyBLS = 1,
    BasicBLS = 2,
    ExtAddr = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MasternodeType {
    Regular = 0,
    HighPerformance = 1,
}
```

### Coinbase Transaction

```rust
#[derive(Debug, Clone)]
pub struct CbTx {
    pub version: CbTxVersion,
    pub height: i32,
    pub merkle_root_mn_list: [u8; 32],
    pub merkle_root_quorums: Option<[u8; 32]>,
    pub best_cl_height_diff: Option<u32>,
    pub best_cl_signature: Option<BlsSignature>,
    pub credit_pool_balance: Option<Amount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CbTxVersion {
    MerkleRootMnList = 1,
    MerkleRootQuorums = 2,
    ClsigAndBalance = 3,
}
```

### LLMQ Quorum Commitment

```rust
#[derive(Debug, Clone)]
pub struct FinalCommitment {
    pub version: u16,
    pub llmq_type: LLMQType,
    pub quorum_hash: [u8; 32],
    pub quorum_index: i16,
    pub signers: Vec<bool>,
    pub valid_members: Vec<bool>,
    pub quorum_public_key: BlsPublicKey,
    pub quorum_vvec_hash: [u8; 32],
    pub quorum_sig: BlsSignature,
    pub members_sig: BlsSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LLMQType {
    Llmq50_60 = 1,
    Llmq400_60 = 2,
    Llmq400_85 = 3,
    Llmq100_67 = 4,
    LlmqTest = 100,
    // ... others
}
```

### Asset Lock/Unlock (Platform)

```rust
#[derive(Debug, Clone)]
pub struct AssetLockTx {
    pub version: u8,
    pub credit_outputs: Vec<TxOut>,
}

#[derive(Debug, Clone)]
pub struct AssetUnlockTx {
    pub version: u8,
    pub index: u64,
    pub fee: Amount,
    pub height: u32,
    pub quorum_hash: [u8; 32],
    pub quorum_sig: BlsSignature,
}
```

## BLS Types (Phase 1 Approach)

**Problem**: BLS cryptography is complex, but we need to parse BLS keys/signatures.

**Solution**: Opaque byte arrays for Phase 1.

```rust
// Phase 1: No validation, just storage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlsPublicKey {
    pub data: Vec<u8>,
    pub legacy: bool, // Legacy vs Basic BLS scheme
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlsSignature {
    pub data: Vec<u8>,
}

// Phase 2+: Real BLS operations using dashbls or bls-signatures crate
```

## Serialization Strategy

### Using `bitcoin::consensus::Encodable`

Extend Bitcoin's serialization traits:

```rust
use bitcoin::consensus::{Encodable, Decodable};

impl Encodable for Transaction {
    fn consensus_encode<W: Write>(&self, writer: &mut W) -> Result<usize> {
        let mut len = 0;

        // Dash combines version and type
        let combined = ((self.tx_type as u32) << 16) | (self.version as u32 & 0xFFFF);
        len += combined.consensus_encode(writer)?;

        len += self.inputs.consensus_encode(writer)?;
        len += self.outputs.consensus_encode(writer)?;
        len += self.lock_time.consensus_encode(writer)?;

        // Extra payload only if special tx
        if self.tx_type != DashTxType::Normal && self.version >= 3 {
            if let Some(payload) = &self.extra_payload {
                len += payload.consensus_encode(writer)?;
            }
        }

        Ok(len)
    }
}

impl Decodable for Transaction {
    fn consensus_decode<R: Read>(reader: &mut R) -> Result<Self> {
        let combined: u32 = Decodable::consensus_decode(reader)?;
        let version = (combined & 0xFFFF) as i16;
        let tx_type = DashTxType::from((combined >> 16) as u16)?;

        let inputs = Decodable::consensus_decode(reader)?;
        let outputs = Decodable::consensus_decode(reader)?;
        let lock_time = Decodable::consensus_decode(reader)?;

        let extra_payload = if tx_type != DashTxType::Normal && version >= 3 {
            Some(ExtraPayload::consensus_decode_with_type(reader, tx_type)?)
        } else {
            None
        };

        Ok(Transaction {
            version,
            tx_type,
            inputs,
            outputs,
            lock_time,
            extra_payload,
        })
    }
}
```

## Testing Strategy

### 1. Unit Tests

Test each type's serialization round-trip:

```rust
#[test]
fn test_transaction_serialization() {
    let tx = Transaction {
        version: 2,
        tx_type: DashTxType::Normal,
        inputs: vec![/* ... */],
        outputs: vec![/* ... */],
        lock_time: 0,
        extra_payload: None,
    };

    let serialized = serialize(&tx);
    let deserialized: Transaction = deserialize(&serialized).unwrap();

    assert_eq!(tx, deserialized);
}
```

### 2. Integration Tests Against Dash Core

**Golden test approach:**

1. Export real transactions from Dash Core (testnet/mainnet)
2. Deserialize with librustdash
3. Re-serialize and compare byte-for-byte

```rust
#[test]
fn test_parse_real_protx() {
    // Real ProRegTx from mainnet block 1234567
    let hex = "030001000174c3..."; // Full hex from dashd
    let bytes = hex::decode(hex).unwrap();

    let tx: Transaction = deserialize(&bytes).unwrap();

    assert_eq!(tx.tx_type, DashTxType::ProviderRegister);

    if let Some(ExtraPayload::ProviderRegister(protx)) = tx.extra_payload {
        assert_eq!(protx.version, ProTxVersion::BasicBLS);
        // ... more assertions
    } else {
        panic!("Wrong payload type");
    }

    // Round-trip test
    let reserialized = serialize(&tx);
    assert_eq!(bytes, reserialized);
}
```

### 3. Property-Based Testing

Use `proptest` or `quickcheck`:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn transaction_roundtrip(version in 1i16..4, lock_time in any::<u32>()) {
        let tx = Transaction {
            version,
            tx_type: DashTxType::Normal,
            inputs: vec![],
            outputs: vec![],
            lock_time,
            extra_payload: None,
        };

        let bytes = serialize(&tx);
        let decoded: Transaction = deserialize(&bytes).unwrap();

        prop_assert_eq!(tx, decoded);
    }
}
```

### 4. Fuzzing

Use `cargo-fuzz` for deserialization:

```rust
#[macro_use] extern crate libfuzzer_sys;

fuzz_target!(|data: &[u8]| {
    let _ = Transaction::deserialize(data);
});
```

## Dependencies

```toml
[dependencies]
# Bitcoin primitives
bitcoin = { version = "0.32", features = ["serde"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
hex = "0.4"
byteorder = "1.5"

# Error handling
thiserror = "1.0"

[dev-dependencies]
# Testing
proptest = "1.0"
quickcheck = "1.0"

# Golden tests
serde_json = "1.0"
```

## Documentation Requirements

### 1. Rustdoc

Every public type and function must have documentation:

```rust
/// A Dash transaction.
///
/// Dash transactions extend Bitcoin transactions with:
/// - A `tx_type` field for special transaction types
/// - An optional `extra_payload` for special transaction data
///
/// # Serialization Format
///
/// ```text
/// - 4 bytes: combined version and type (type << 16 | version)
/// - varint: input count
/// - inputs: [TxIn]
/// - varint: output count
/// - outputs: [TxOut]
/// - 4 bytes: lock_time
/// - optional: extra_payload (if type != NORMAL and version >= 3)
/// ```
pub struct Transaction {
    // ...
}
```

### 2. Examples

Each crate should have examples:

```rust
// examples/parse_block.rs
use librustdash::primitives::Block;

fn main() {
    let hex = "..."; // Block hex
    let bytes = hex::decode(hex).unwrap();
    let block = Block::deserialize(&bytes).unwrap();

    println!("Block hash: {}", block.hash());
    println!("Transactions: {}", block.transactions.len());
}
```

### 3. Book/Guide

Create a mdBook-style guide:

```
docs/
├── introduction.md
├── serialization.md
├── special-transactions.md
├── testing.md
└── SUMMARY.md
```

## Performance Considerations

### 1. Zero-Copy Where Possible

```rust
// Instead of copying
pub struct Transaction {
    pub script_sig: Vec<u8>, // Copies data
}

// Use slices or Cow
pub struct Transaction<'a> {
    pub script_sig: Cow<'a, [u8]>, // Can reference or own
}
```

### 2. Lazy Hashing

```rust
pub struct Transaction {
    data: Vec<u8>,
    cached_hash: OnceCell<Txid>,
}

impl Transaction {
    pub fn txid(&self) -> Txid {
        *self.cached_hash.get_or_init(|| {
            // Compute hash only once
            compute_hash(&self.data)
        })
    }
}
```

### 3. Benchmarking

Use `criterion`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_deserialize_transaction(c: &mut Criterion) {
    let data = hex::decode("...").unwrap();

    c.bench_function("deserialize transaction", |b| {
        b.iter(|| Transaction::deserialize(black_box(&data)))
    });
}

criterion_group!(benches, bench_deserialize_transaction);
criterion_main!(benches);
```

## Migration Path

### Phase 1: Primitives (Weeks 1-4)
- [x] Design document
- [ ] Project scaffolding
- [ ] Basic types (Block, Transaction, TxIn, TxOut)
- [ ] Serialization/deserialization
- [ ] DashTxType enum
- [ ] Simple payload types (CbTx, AssetLock)

### Phase 2: Complex Payloads (Weeks 5-8)
- [ ] ProRegTx and variants
- [ ] LLMQ commitment
- [ ] All payload types complete
- [ ] Comprehensive golden tests
- [ ] Property-based tests

### Phase 3: Validation Helpers (Weeks 9-12)
- [ ] Hash computation
- [ ] Merkle tree utilities
- [ ] BLS signature placeholders
- [ ] Network-specific constants

### Phase 4: Polish (Weeks 13-16)
- [ ] Documentation
- [ ] Examples
- [ ] Benchmarks
- [ ] Fuzzing
- [ ] 1.0.0 release

## Success Metrics

**Phase 1:**
- Can parse 100% of mainnet blocks
- Can parse 100% of special transaction types
- Round-trip serialization matches byte-for-byte
- Test coverage > 80%

**Phase 2:**
- Used by dash-indexer successfully
- Used by at least one other project (wallet, explorer, etc.)
- Zero serialization bugs reported
- Performance within 10% of C++ (deserialization)

## Open Questions

1. **BLS integration**: Should Phase 1 vendor dashbls, or wait?
   - **Recommendation**: Wait. Use opaque byte arrays. Add BLS validation in Phase 2.

2. **Async support**: Should serialization be async-aware?
   - **Recommendation**: No. Keep it synchronous. Async is for I/O, not parsing.

3. **no_std support**: Should we support embedded/no_std?
   - **Recommendation**: Not Phase 1. Could add later with feature flags.

4. **WASM support**: Should this compile to WASM?
   - **Recommendation**: Yes, but not a blocker. Use `wasm-bindgen` compatible deps.

5. **Version policy**: When do we break API compatibility?
   - **Recommendation**: Follow semver strictly. Major version = breaking changes.

## References

- [librustzcash Architecture](https://github.com/zcash/librustzcash)
- [Bitcoin Rust Crate](https://github.com/rust-bitcoin/rust-bitcoin)
- [Dash Core Source](https://github.com/dashpay/dash)
- [Dash Transaction Types](https://github.com/dashpay/dash/blob/develop/src/primitives/transaction.h)
- [Dash Special Transactions](https://github.com/dashpay/dash/tree/develop/src/evo)

## Appendix: Complete Type Hierarchy

```
librustdash::primitives
├── Block
│   ├── header: BlockHeader
│   └── transactions: Vec<Transaction>
├── BlockHeader
│   ├── version: i32
│   ├── prev_blockhash: BlockHash
│   ├── merkle_root: TxMerkleNode
│   ├── time: u32
│   ├── bits: u32
│   └── nonce: u32
├── Transaction
│   ├── version: i16
│   ├── tx_type: DashTxType
│   ├── inputs: Vec<TxIn>
│   ├── outputs: Vec<TxOut>
│   ├── lock_time: u32
│   └── extra_payload: Option<ExtraPayload>
├── TxIn
│   ├── previous_output: OutPoint
│   ├── script_sig: Script
│   └── sequence: u32
├── TxOut
│   ├── value: Amount
│   └── script_pubkey: Script
└── ExtraPayload (enum)
    ├── ProviderRegister(ProRegTx)
    ├── ProviderUpdateService(ProUpServTx)
    ├── ProviderUpdateRegistrar(ProUpRegTx)
    ├── ProviderUpdateRevoke(ProUpRevTx)
    ├── Coinbase(CbTx)
    ├── QuorumCommitment(FinalCommitment)
    ├── MnhfSignal(MnhfTx)
    ├── AssetLock(AssetLockTx)
    └── AssetUnlock(AssetUnlockTx)
```
