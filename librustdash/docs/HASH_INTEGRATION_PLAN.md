# librustdash Hash Integration & Indexer Preparation Plan

**Date:** December 31, 2025
**Status:** Planning
**Next Goal:** Block Indexer (Zaino equivalent)

## Context

librustdash follows the librustzcash pattern - a **primitives library** providing core data structures for higher-level projects:
- **Block indexer** (Zaino equivalent) - **IMMEDIATE NEXT GOAL**
- Core daemon/validator (Zebra equivalent) - Future
- Wallet libraries (Zallet equivalent) - Future Phase 2

**Current status:** Phase 1 complete (serialization/deserialization), 105 tests passing, all 9 special transaction payloads implemented.

**Immediate goal:** Integrate `bitcoin_hashes` to prepare primitives for block indexer work.

---

## Verified Facts from Dash Source

From `/Users/nathan/projects/dash/src/evo/dmn_types.h`:
- **Regular masternode collateral: 1000 DASH**
- **Evo masternode collateral: 4000 DASH**
- Voting weight: Regular=1, Evo=4

---

## Current Dependencies

From `Cargo.toml`:
- `bitcoin = { version = "0.32", features = ["serde"] }` - **Already included**
- `sha2 = "0.10"` - **Present but UNUSED** (can be removed)
- `byteorder = "1.5"` - Used for serialization
- `thiserror = "1.0"` - Error handling

**Key insight:** We already have access to `bitcoin_hashes` through the `bitcoin` crate dependency.

---

## Hash Integration Plan

### Phase 1.5: Add Hash Utilities (1-2 days)

#### Goals
1. Leverage `bitcoin_hashes` from existing `bitcoin` crate
2. Add hash computation methods for indexer needs
3. Remove unused `sha2` dependency
4. **Maintain backward compatibility** (no breaking changes to existing types)

#### Files to Create/Modify

**New file: `src/hash.rs`** (~150 lines)
```rust
//! Hash computation utilities for Dash blockchain primitives
//!
//! Uses bitcoin_hashes for compatibility with Bitcoin ecosystem

use bitcoin::hashes::{sha256d, Hash};

// Re-export hash types
pub use bitcoin::hashes::sha256d::Hash as Sha256dHash;

/// Compute double SHA-256 hash (Bitcoin/Dash standard)
pub fn sha256d(data: &[u8]) -> Sha256dHash {
    Sha256dHash::hash(data)
}

/// Compute merkle root from transaction hashes
///
/// Implements Bitcoin/Dash merkle tree algorithm with duplicate handling
/// for odd number of hashes.
pub fn compute_merkle_root(hashes: &[Sha256dHash]) -> Sha256dHash {
    if hashes.is_empty() {
        return Sha256dHash::all_zeros();
    }

    let mut level = hashes.to_vec();

    while level.len() > 1 {
        let mut next_level = Vec::new();

        for chunk in level.chunks(2) {
            let combined = if chunk.len() == 2 {
                [chunk[0].as_ref(), chunk[1].as_ref()].concat()
            } else {
                // Duplicate last hash if odd number
                [chunk[0].as_ref(), chunk[0].as_ref()].concat()
            };
            next_level.push(Sha256dHash::hash(&combined));
        }

        level = next_level;
    }

    level[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256d_known_value() {
        // Golden test with known Dash block hash
    }

    #[test]
    fn test_merkle_root_single() {
        // Single hash should return itself
    }

    #[test]
    fn test_merkle_root_odd() {
        // Test duplicate handling for odd number of hashes
    }
}
```

**Modify: `src/transaction.rs`** (add methods)
```rust
use crate::hash::{sha256d, Sha256dHash};

impl Transaction {
    /// Compute transaction ID (double SHA-256 of serialized transaction)
    ///
    /// # Example
    /// ```
    /// use librustdash::Transaction;
    /// let tx = Transaction { /* ... */ };
    /// let txid = tx.txid()?;
    /// println!("Transaction ID: {}", txid);
    /// ```
    pub fn txid(&self) -> Result<Sha256dHash> {
        let serialized = self.serialize()?;
        Ok(sha256d(&serialized))
    }

    /// Compute inputs hash for ProRegTx replay protection
    ///
    /// Used by ProRegTx, ProUpRegTx, ProUpServTx, ProUpRevTx to prevent
    /// replay attacks by hashing the transaction inputs.
    pub fn compute_inputs_hash(&self) -> Result<[u8; 32]> {
        use crate::serialize::write_compact_size;

        let mut buf = Vec::new();
        write_compact_size(&mut buf, self.inputs.len() as u64)?;
        for input in &self.inputs {
            input.serialize(&mut buf)?;
        }

        let hash = sha256d(&buf);
        Ok(hash.to_byte_array())
    }
}
```

**Modify: `src/block.rs`** (add methods)
```rust
use crate::hash::{compute_merkle_root, sha256d, Sha256dHash};

impl BlockHeader {
    /// Compute block hash (double SHA-256 of 80-byte header)
    ///
    /// # Example
    /// ```
    /// use librustdash::BlockHeader;
    /// let header = BlockHeader { /* ... */ };
    /// let block_hash = header.hash()?;
    /// println!("Block hash: {}", block_hash);
    /// ```
    pub fn hash(&self) -> Result<Sha256dHash> {
        let serialized = self.serialize()?;
        debug_assert_eq!(serialized.len(), 80, "Block header must be 80 bytes");
        Ok(sha256d(&serialized))
    }
}

impl Block {
    /// Compute merkle root from transaction hashes
    ///
    /// Computes the merkle root by hashing all transactions and building
    /// a merkle tree following Bitcoin/Dash algorithm.
    pub fn compute_merkle_root(&self) -> Result<Sha256dHash> {
        let txids: Result<Vec<_>> = self.transactions
            .iter()
            .map(|tx| tx.txid())
            .collect();

        Ok(compute_merkle_root(&txids?))
    }

    /// Verify that header merkle root matches computed value
    ///
    /// Returns true if the merkle root in the block header matches
    /// the computed merkle root from transactions.
    pub fn verify_merkle_root(&self) -> Result<bool> {
        let computed = self.compute_merkle_root()?;
        let header_root = Sha256dHash::from_slice(&self.header.merkle_root)?;
        Ok(computed == header_root)
    }
}
```

**Modify: `src/lib.rs`**
```rust
pub mod hash;

// Re-export hash types for convenience
pub use hash::{compute_merkle_root, sha256d, Sha256dHash};
```

**Modify: `Cargo.toml`**
```toml
[dependencies]
bitcoin = { version = "0.32", features = ["serde"] }
byteorder = "1.5"
thiserror = "1.0"
# REMOVED: sha2 = "0.10"  # Unused, bitcoin_hashes provides this

[dev-dependencies]
hex = "0.4"
proptest = "1.5"
serde_json = "1.0"
```

#### Implementation Strategy

1. **Do NOT change existing struct fields** - Keep `[u8; 32]` in OutPoint, BlockHeader, etc.
2. **Add new methods only** - `txid()`, `hash()`, `compute_merkle_root()`
3. **Use bitcoin_hashes types for return values** - Better type safety, Display formatting
4. **Maintain backward compatibility** - All 105 existing tests must pass without changes

#### Testing Requirements

- Add golden tests with real Dash block hashes
- Verify merkle root computation matches Dash Core
- Test against known mainnet block (e.g., genesis block, block 100000)
- Round-trip: serialize → hash → verify
- Test merkle tree with odd/even number of transactions

---

## What a Block Indexer Needs

### Core Primitives (Already Have ✓)
- ✓ Block deserialization (BlockHeader, Block)
- ✓ Transaction deserialization (all 10 types)
- ✓ Special transaction payload parsing (9 payload types)
- ✓ BLS types (opaque bytes for now)
- ✓ BitVector for LLMQ quorums
- ✓ Network address types (IPv4/IPv6)

### Hash Operations (Adding Now)
- Transaction hash computation (txid)
- Block hash computation
- Merkle root computation/verification

### Additional Needs (Future Work)
- Block chain validation (prev_blockhash linking)
- Difficulty/chainwork calculation
- Block file format parsing (.dat files with magic bytes)
- Network message parsing (optional, depends on sync strategy)
- ZMQ message parsing (for real-time updates from dashd)

---

## Library Organization Strategy

### Current Structure (Single Crate)
```
librustdash/
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── serialize.rs
│   ├── tx_type.rs
│   ├── transaction.rs
│   ├── block.rs
│   ├── bls.rs
│   ├── bitvector.rs
│   ├── network_info.rs
│   ├── hash.rs (NEW)
│   └── payloads/ (9 payload files)
├── tests/
├── docs/
└── Cargo.toml
```

**Status:** Good for now. Primitives are cohesive in a single crate.

### Future Workspace Structure (Post-Indexer)

Following librustzcash pattern (when we have 3+ crates):

```
librustdash/ (workspace root)
├── Cargo.toml (workspace manifest)
├── dash_primitives/ (current librustdash crate)
│   ├── src/ (block, tx, payloads, serialization)
│   └── Cargo.toml
├── dash_consensus/ (validation rules)
│   ├── src/ (consensus parameters, validation)
│   └── Cargo.toml
├── dash_script/ (Bitcoin script execution)
│   └── Cargo.toml
├── components/
│   ├── dash_address/ (Address encoding/decoding)
│   ├── dash_encoding/ (Common encoding utilities)
│   └── dash_constants/ (Network parameters)
└── README.md
```

**Timeline:** Only create workspace when we have 3+ crates. For indexer work, keep single crate.

---

## Re-exporting Bitcoin Functionality

### What to Re-export from bitcoin Crate

```rust
// In src/lib.rs or new src/bitcoin_compat.rs

// Address types (for indexer output addresses)
pub use bitcoin::address::Address;
pub use bitcoin::network::Network;

// Script types (already used internally)
pub use bitcoin::script::Script;

// Amount type (better than raw i64)
pub use bitcoin::amount::Amount;

// Hash types
pub use bitcoin::hashes::{sha256d, Hash};
```

**Benefits:**
- Don't reinvent address encoding (Base58Check, Bech32)
- Consistent with Bitcoin ecosystem
- Just need to set Dash-specific network constants

**Dash-specific overrides needed:**
- Network magic bytes: `0xBF0C6BBD` (mainnet), `0xFFCAE2CE` (testnet)
- BIP44 coin type: **5** (vs Bitcoin's 0)
- Address version bytes:
  - Mainnet P2PKH: `0x4C` (76 decimal)
  - Mainnet P2SH: `0x10` (16 decimal)
  - Testnet P2PKH: `0x8C` (140 decimal)
  - Testnet P2SH: `0x13` (19 decimal)

**Strategy:** Re-export base types, document Dash-specific parameters in constants module.

---

## Indexer Primitive Requirements

### What Zaino Uses from librustzcash

From librustzcash structure:
- `zcash_primitives` - Block/tx types ✓ (we have equivalent)
- `zcash_protocol` - Protocol constants ✗ (need dash_consensus crate)
- `zcash_encoding` - Compact size ✓ (we have this)
- `zcash_client_backend` - Indexer logic ✗ (future dash-indexer crate)

### What dash-indexer Would Need from librustdash

**Block Processing:**
```rust
use librustdash::{Block, BlockHeader, Transaction};

// Parse block from .dat file or network
let block = Block::deserialize(&block_bytes)?;

// Compute block hash for indexing
let block_hash = block.header.hash()?;

// Verify block integrity
assert!(block.verify_merkle_root()?);

// Index all transactions
for tx in &block.transactions {
    let txid = tx.txid()?;
    // Store tx in database with txid as key
}
```

**Special Transaction Handling:**
```rust
use librustdash::{DashTxType, ProRegTx, CbTx};

match tx.tx_type {
    DashTxType::ProviderRegister => {
        // Extract masternode registration
        let protx = ProRegTx::deserialize(&tx.extra_payload)?;
        // Track new masternode with 1000 or 4000 DASH collateral
    }
    DashTxType::Coinbase => {
        // Extract coinbase info
        let cbtx = CbTx::deserialize(&tx.extra_payload)?;
        // Track masternode list merkle root, chainlock, credit pool
    }
    // Handle other special types...
    _ => {}
}
```

**Address Extraction:**
```rust
use librustdash::TxOut;

for output in &tx.outputs {
    // Parse scriptPubKey to extract address
    // (will need bitcoin::script utilities)
    let address = extract_address(&output.script_pubkey)?;
    // Index by address for balance queries
}
```

### Additional Crates Needed for Indexer

**dash-consensus (separate crate):**
```rust
// Consensus parameters
pub struct ConsensusParams {
    pub network: Network,
    pub pow_limit: u256,
    pub difficulty_adjustment_interval: u32,
    // Dash-specific
    pub deterministic_mn_enabled_height: u32,
    pub basic_bls_enabled_height: u32,
    pub v20_enabled_height: u32,
}

// Block validation
pub fn validate_block(block: &Block, params: &ConsensusParams) -> Result<()>;
pub fn calculate_next_work_required(prev: &BlockHeader, params: &ConsensusParams) -> u32;
```

**dash-indexer (separate repo/crate):**
```rust
// Database abstraction
pub trait IndexStore {
    fn store_block(&mut self, hash: Sha256dHash, block: &Block) -> Result<()>;
    fn get_block(&self, hash: &Sha256dHash) -> Result<Option<Block>>;
    fn store_tx(&mut self, txid: Sha256dHash, tx: &Transaction) -> Result<()>;
    fn get_tx(&self, txid: &Sha256dHash) -> Result<Option<Transaction>>;
    fn index_address(&mut self, addr: &Address, txid: Sha256dHash) -> Result<()>;
}

// LMDB or RocksDB backend
pub struct LmdbIndexStore { /* ... */ }
impl IndexStore for LmdbIndexStore { /* ... */ }
```

---

## Key Dash Features for Indexer

### InstantSend (Priority Feature)
**What it is:** LLMQ-based transaction locking for instant confirmations (2-3 seconds)

**Indexer requirements:**
- Track InstantSend locks (quorum signatures on transactions)
- Mark transactions as "locked" vs "unlocked"
- Verify quorum signatures (needs BLS validation eventually)

**Primitives needed:**
- QuorumCommitmentPayload ✓ (already have)
- BLS signature types ✓ (already have as opaque bytes)
- InstantSend lock message parsing ✗ (future, not in blockchain data)

**Note:** InstantSend locks come via P2P/ZMQ messages, not in blocks. For indexer, we can track which transactions are in locked blocks (via ChainLocks).

### ChainLocks
**What it is:** LLMQ-based block finality (prevents 51% attacks, enables InstantSend)

**Indexer requirements:**
- Parse ChainLock signatures from CbTx
- Track best chainlocked block height
- Reorganizations only valid up to chainlocked block

**Primitives needed:**
- CbTx v3 with `best_cl_signature` ✓ (already have)
- BLS signature validation ✗ (future, can parse now)

### Deterministic Masternode List
**What it is:** On-chain masternode registry with cryptographic proofs

**Indexer requirements:**
- Track masternode registrations (ProRegTx)
  - Regular: 1000 DASH collateral
  - Evo: 4000 DASH collateral
- Track updates (ProUpServTx, ProUpRegTx, ProUpRevTx)
- Build deterministic masternode list at any block height
- Track masternode states (ENABLED, POSE_BANNED, REMOVED)

**Primitives needed:**
- All ProTx types ✓ (already have)
- Collateral validation (check for exact 1000 or 4000 DASH output)

### Platform (Dash Evolution)
**What it is:** Platform chain for usernames, contracts, data storage

**Indexer requirements:**
- Track asset locks (DASH → Platform credits)
- Track asset unlocks (Platform credits → DASH)
- Maintain credit pool balance
- Platform node registry (Evo masternodes)

**Primitives needed:**
- AssetLockPayload ✓ (already have)
- AssetUnlockPayload ✓ (already have)
- CbTx v3 `credit_pool_balance` ✓ (already have)
- ProRegTx v2+ with platform fields ✓ (already have)

---

## Implementation Timeline

### Week 1: Hash Integration (2-3 days)
**Goal:** Add hash utilities, maintain backward compatibility

**Tasks:**
1. Create `src/hash.rs` with `sha256d()` and `compute_merkle_root()`
2. Add `Transaction::txid()` method
3. Add `BlockHeader::hash()` method
4. Add `Block::compute_merkle_root()` and `verify_merkle_root()`
5. Add golden tests with real Dash block hashes
6. Update `Cargo.toml` to remove unused `sha2`
7. Verify all 105 tests still pass
8. Add 5-10 new hash-specific tests

**Success Criteria:**
- Can compute txid for any transaction
- Can compute block hash for any block
- Can verify merkle roots
- Golden test with mainnet block hash matches Dash Core
- All existing tests pass
- Zero breaking changes

### Week 2: Documentation & Indexer Planning (2-3 days)
**Goal:** Prepare for indexer development

**Tasks:**
1. Update `README.md` with hash examples
2. Add rustdoc examples to hash module
3. Create `docs/INDEXER_REQUIREMENTS.md`
4. Create `docs/INDEXER_DESIGN.md`
5. Research LMDB vs RocksDB for storage backend
6. Define gRPC API for indexer queries

**Deliverables:**
- Updated documentation
- Clear requirements doc for indexer
- Architectural design for dash-indexer

### Week 3-4: Indexer Skeleton (New Crate/Repo)
**Goal:** Create dash-indexer crate structure

**Location:** `/Users/nathan/projects/dash-indexer` (separate repo)

**Dependencies:**
```toml
[dependencies]
librustdash = { version = "0.1", path = "../librustdash" }
bitcoin = "0.32"
lmdb = "0.8"  # Or rocksdb = "0.21"
tokio = { version = "1.0", features = ["full"] }
tonic = "0.10"  # gRPC server
prost = "0.12"  # Protocol buffers
```

**Initial structure:**
```
dash-indexer/
├── src/
│   ├── main.rs (CLI entry point)
│   ├── indexer.rs (Core indexer logic)
│   ├── storage/ (Database abstraction)
│   │   ├── mod.rs
│   │   ├── lmdb.rs (LMDB backend)
│   │   └── memory.rs (In-memory for testing)
│   ├── sync/ (Block synchronization)
│   │   ├── mod.rs
│   │   ├── file.rs (Read from blk*.dat files)
│   │   └── zmq.rs (Real-time updates from dashd)
│   └── rpc/ (gRPC API server)
│       ├── mod.rs
│       └── api.proto
├── Cargo.toml
└── README.md
```

**Out of scope:** Full indexer implementation (future work, ~2-3 months)

---

## Critical Files Modified (Hash Integration)

### Created
- `src/hash.rs` - Hash computation utilities (~150 lines)
- `tests/hash_tests.rs` - Golden tests for hashes (~100 lines)

### Modified
- `src/lib.rs` - Add hash module and re-exports (~5 lines)
- `src/transaction.rs` - Add `txid()`, `compute_inputs_hash()` (~30 lines)
- `src/block.rs` - Add `hash()`, `compute_merkle_root()`, `verify_merkle_root()` (~40 lines)
- `Cargo.toml` - Remove `sha2` dependency (~1 line)
- `README.md` - Update with hash examples (~20 lines)

### Test Files
- `tests/golden_tests.rs` - Add block hash golden tests (~50 lines)

**Total new code:** ~200-250 lines
**Total modified code:** ~95 lines
**Effort:** 2-3 days

---

## Non-Goals

### NOT doing in this phase:
- ✗ Wallet functionality (Phase 2, 6+ months away)
- ✗ CoinJoin support (not priority per user feedback)
- ✗ BLS signature verification (future, keep opaque bytes for now)
- ✗ ECDSA signature verification (not needed for indexer)
- ✗ Script execution (not needed for basic indexing)
- ✗ Breaking changes to existing types
- ✗ Workspace restructuring (wait until 3+ crates)
- ✗ Network message parsing (can use ZMQ from dashd instead)

### Doing in this phase:
- ✓ Hash utilities (txid, block hash, merkle root)
- ✓ Prepare primitives for indexer
- ✓ Remove unused dependencies
- ✓ Maintain backward compatibility
- ✓ Documentation updates
- ✓ Golden tests with real Dash data

---

## Success Metrics

### Hash Integration Complete When:
- [ ] Can compute txid for any Dash transaction
- [ ] Can compute block hash for any block
- [ ] Can compute and verify merkle roots
- [ ] Golden test with real mainnet block passes (e.g., block 1000000)
- [ ] All 105 existing tests still pass
- [ ] 5+ new hash-specific tests added
- [ ] `sha2` dependency removed from Cargo.toml
- [ ] Documentation updated with hash examples
- [ ] Zero breaking changes to public API
- [ ] rustdoc examples compile and run

### Ready for Indexer Development When:
- [ ] All hash utilities working and tested
- [ ] Can parse any mainnet/testnet block
- [ ] Can extract all special transaction types
- [ ] Documentation of indexer requirements complete
- [ ] Indexer design document written
- [ ] Storage backend chosen (LMDB vs RocksDB)

---

## References

- **Dash Core source:** `/Users/nathan/projects/dash/`
- **librustzcash reference:** `/Users/nathan/projects/librustzcash/`
- **Dash masternode types:** `/Users/nathan/projects/dash/src/evo/dmn_types.h`
- **Zaino indexer:** https://github.com/zingolabs/zaino
- **rust-bitcoin hashes:** https://docs.rs/bitcoin/latest/bitcoin/hashes/
- **LMDB Rust bindings:** https://docs.rs/lmdb/
- **RocksDB Rust bindings:** https://docs.rs/rocksdb/

---

**Last Updated:** December 31, 2025
**Author:** Planning session with Claude Code
**Status:** Ready for implementation
