# librustdash Architecture & Crate Organization

**Goal:** Build a modular Rust ecosystem for Dash, following the librustzcash pattern.

---

## Core Principle: Separation of Concerns

Following librustzcash's design:

```
zcash_protocol (constants)  →  zcash_primitives (data structures)  →  higher-level crates
```

**For Dash:**
```
dash_protocol (constants)  →  librustdash (primitives)  →  dash_indexer, dash_validator, etc.
```

---

## Crate Breakdown

### 1. `librustdash` (primitives) - **CURRENT CRATE**

**Purpose:** Pure data structures and serialization. No validation logic.

**Contains:**
- Block, BlockHeader, Transaction types
- All 9 special transaction payload types
- Serialization/deserialization (CompactSize, etc.)
- Hash utilities (sha256d, txid, merkle root computation)
- BLS types (opaque bytes for now)
- BitVector, network address types

**Does NOT contain:**
- ❌ POW verification
- ❌ Difficulty calculation
- ❌ Block validation rules
- ❌ Transaction validation (beyond parsing)
- ❌ Network constants (those go in dash_protocol)

**Dependencies:**
- `bitcoin` (for hashes, script types, address encoding)
- `byteorder` (serialization)
- `thiserror` (errors)

**Analogy:** Like `zcash_primitives` - just the data structures.

---

### 2. `dash_protocol` (constants) - **NEXT CRATE TO CREATE**

**Purpose:** Network parameters and consensus constants. Very lightweight, no logic.

**Contains:**
```rust
// src/constants/mainnet.rs
pub const MAGIC_BYTES: [u8; 4] = [0xBF, 0x0C, 0x6B, 0xBD];
pub const COIN_TYPE: u32 = 5; // BIP44
pub const P2PKH_VERSION: u8 = 0x4C; // 76
pub const P2SH_VERSION: u8 = 0x10; // 16

// Genesis block
pub const GENESIS_HASH: [u8; 32] = [...];
pub const GENESIS_TIME: u32 = 1390095618;

// Fork activation heights
pub const DIP0003_HEIGHT: u32 = 1028160; // Deterministic MN list
pub const BLS_ACTIVATION_HEIGHT: u32 = 1047200; // Basic BLS
pub const V20_ACTIVATION_HEIGHT: u32 = ...; // Platform

// src/constants/testnet.rs
pub const MAGIC_BYTES: [u8; 4] = [0xFF, 0xCA, 0xE2, 0xCE];
// ... testnet constants

// src/constants/regtest.rs
pub const MAGIC_BYTES: [u8; 4] = [0xFC, 0xC1, 0xB7, 0xDC];
// ... regtest constants

// src/constants/devnet.rs
// ... devnet support

// src/consensus.rs
#[derive(Debug, Clone, Copy)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
    Devnet,
}

pub struct ConsensusParams {
    pub network: Network,
    pub pow_limit: u256,
    pub pow_target_spacing: u32, // 2.5 minutes
    pub pow_target_timespan: u32, // 1 day
    pub difficulty_adjustment_interval: u32,
    pub subsidy_halving_interval: u32,
    // Dash-specific
    pub dip0003_height: u32,
    pub bls_activation_height: u32,
    pub v20_activation_height: u32,
}

impl ConsensusParams {
    pub fn mainnet() -> Self { /* ... */ }
    pub fn testnet() -> Self { /* ... */ }
    pub fn regtest() -> Self { /* ... */ }
}
```

**Does NOT contain:**
- ❌ Validation logic
- ❌ POW calculation
- ❌ Any actual verification

**Dependencies:**
- Minimal (maybe just `hex` for encoding constants)

**Analogy:** Like `zcash_protocol` - just constants and types, no logic.

---

### 3. `dash_consensus` (validation) - **FUTURE CRATE**

**Purpose:** Consensus validation rules. This is where POW goes.

**Contains:**
```rust
// src/pow.rs
/// X11 proof-of-work hash algorithm
pub fn x11_hash(header: &[u8; 80]) -> [u8; 32] {
    // Dash uses X11 (11 chained hash functions)
    // blake, bmw, groestl, jh, keccak, skein, luffa, cubehash, shavite, simd, echo
}

/// Verify block meets POW target
pub fn verify_pow(header: &BlockHeader, params: &ConsensusParams) -> Result<()> {
    let hash = x11_hash(&header.serialize()?);
    let target = compact_to_target(header.bits)?;

    if u256::from_le_bytes(hash) > target {
        return Err(PowError::InsufficientWork);
    }
    Ok(())
}

/// Calculate next required difficulty
pub fn get_next_work_required(
    last_block: &BlockHeader,
    prev_timestamps: &[u32],
    params: &ConsensusParams,
) -> Result<u32> {
    // Dark Gravity Wave v3 difficulty adjustment
    // https://github.com/dashpay/dash/blob/master/src/pow.cpp
}

// src/validation.rs
/// Full block validation
pub fn validate_block(
    block: &Block,
    prev_header: &BlockHeader,
    params: &ConsensusParams,
) -> Result<()> {
    // 1. Verify POW
    verify_pow(&block.header, params)?;

    // 2. Verify merkle root
    block.verify_merkle_root()?;

    // 3. Verify timestamp rules
    verify_timestamp(&block.header, prev_header)?;

    // 4. Verify coinbase is first transaction
    // 5. Verify all transactions
    // 6. Validate special transactions (ProRegTx collateral, etc.)

    Ok(())
}

/// Validate special transaction specific rules
pub fn validate_protx(
    tx: &Transaction,
    protx: &ProRegTx,
    utxo_set: &UtxoSet,
) -> Result<()> {
    // Check collateral is exactly 1000 or 4000 DASH
    let expected_collateral = match protx.mn_type {
        MasternodeType::Regular => 1000 * COIN,
        MasternodeType::Evo => 4000 * COIN,
    };

    // Verify collateral UTXO exists and has correct amount
    // Verify operator key is unique
    // Verify addresses are unique
    // etc.
}
```

**Dependencies:**
- `librustdash` (primitives)
- `dash_protocol` (constants)
- X11 hash crate (might need `blake-hash`, `groestl`, etc.)

**When to build:** After indexer is working. Not needed for basic indexing.

**Analogy:** zcashd has POW validation, but it's NOT in librustzcash primitives.

---

## Who Needs What?

### Block Indexer (dash-indexer)
**Needs:**
- ✓ librustdash (parse blocks/transactions)
- ✓ dash_protocol (know network parameters, activation heights)
- ✗ dash_consensus (can trust dashd for validation)

**Why:** Indexer just stores and queries blockchain data. It doesn't need to validate - dashd already did.

### Full Validator (dash-validator - future Zebra equivalent)
**Needs:**
- ✓ librustdash (parse blocks/transactions)
- ✓ dash_protocol (network parameters)
- ✓ dash_consensus (POW, difficulty, full validation)

**Why:** Full node needs to independently verify everything.

### Wallet (dash-wallet - future)
**Needs:**
- ✓ librustdash (build transactions)
- ✓ dash_protocol (network parameters, fees)
- ✓ dash_keys (HD derivation, signing)
- ✗ dash_consensus (doesn't validate blocks)

**Why:** Wallet needs to create valid transactions, not validate the chain.

---

## Recommended Implementation Order

### Phase 1: Primitives (DONE ✓)
- librustdash with all payload types
- Hash utilities (sha256d, txid, merkle root)

### Phase 2: Constants (1-2 weeks)
- Create `dash_protocol` crate
- Extract constants (network magic, coin type, activation heights)
- Keep it lightweight (no validation logic)

### Phase 3: Indexer (2-3 months)
- Create `dash-indexer` as separate repo/crate
- Depends on: librustdash + dash_protocol
- Storage (LMDB or RocksDB)
- Sync from dashd (ZMQ or block files)
- gRPC query API

### Phase 4: Consensus (3-4 months) - OPTIONAL
- Create `dash_consensus` crate
- X11 POW implementation
- Dark Gravity Wave difficulty adjustment
- Full block/transaction validation
- Only needed for dash-validator (Zebra equivalent)

---

## Where Does POW Go?

**Answer: In `dash_consensus`, NOT in `librustdash`.**

**Rationale:**
1. **Separation of concerns** - Primitives = data, Consensus = validation
2. **Optional dependency** - Indexer doesn't need POW
3. **Follows zcash pattern** - They don't have POW in primitives either
4. **Maintainability** - Easier to update validation rules separately

**What librustdash DOES provide:**
- `BlockHeader::hash()` - Compute the block hash (for indexing, lookups)
- `BlockHeader::serialize()` - Get 80-byte header for hashing

**What dash_consensus WILL provide:**
- `x11_hash()` - Compute X11 POW hash
- `verify_pow()` - Check if block meets target
- `get_next_work_required()` - Difficulty adjustment

**Example usage:**
```rust
// In dash_consensus:
use librustdash::BlockHeader;

pub fn verify_pow(header: &BlockHeader, target: u256) -> Result<()> {
    // Serialize the header (librustdash provides this)
    let header_bytes = header.serialize()?;

    // Compute X11 hash (dash_consensus provides this)
    let hash = x11_hash(&header_bytes);

    // Check against target
    if u256::from_le_bytes(hash) > target {
        return Err(PowError::InsufficientWork);
    }
    Ok(())
}
```

---

## Workspace Structure (Future)

Once we have 3+ crates:

```
librustdash/ (workspace root)
├── Cargo.toml (workspace manifest)
│
├── dash_primitives/ (rename from librustdash)
│   ├── src/
│   │   ├── block.rs
│   │   ├── transaction.rs
│   │   ├── payloads/
│   │   ├── hash.rs
│   │   └── ...
│   └── Cargo.toml
│
├── components/
│   ├── dash_protocol/
│   │   ├── src/
│   │   │   ├── constants/
│   │   │   │   ├── mainnet.rs
│   │   │   │   ├── testnet.rs
│   │   │   │   └── regtest.rs
│   │   │   ├── consensus.rs
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── dash_address/
│   │   └── (Address encoding/decoding with Dash parameters)
│   │
│   └── dash_encoding/
│       └── (Common encoding utilities)
│
├── dash_consensus/ (optional, for validators)
│   ├── src/
│   │   ├── pow.rs (X11 hash, POW verification)
│   │   ├── validation.rs (Block/tx validation)
│   │   └── difficulty.rs (Dark Gravity Wave)
│   └── Cargo.toml
│
└── README.md
```

**Timeline:** Create workspace when we have 3+ crates (after dash_protocol is created).

---

## Summary

**High-level answer to "where does POW go?":**

1. **librustdash (primitives)** - Data structures + serialization + hash utilities
   - `BlockHeader::hash()` ✓
   - `BlockHeader::serialize()` ✓
   - POW verification ✗

2. **dash_protocol (constants)** - Network parameters, activation heights
   - Magic bytes, coin type, genesis blocks ✓
   - Activation heights for forks ✓
   - POW verification ✗

3. **dash_consensus (validation)** - Validation rules
   - X11 hash algorithm ✓
   - POW verification ✓
   - Difficulty adjustment ✓
   - Full block/tx validation ✓

**For the indexer:** You only need #1 and #2. Skip #3 entirely.

**For a full validator:** You need all three.

This follows the zcash pattern exactly:
- `zcash_protocol` = constants
- `zcash_primitives` = data structures
- Validation is elsewhere (zcashd or Zebra)

---

**Last Updated:** December 31, 2025
**Status:** Architectural guidance
