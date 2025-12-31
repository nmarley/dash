# librustdash Phase 1 Implementation Plan - Test-Driven Development

**Document Version:** 2.0  
**Date:** December 31, 2025  
**Status:** Ready to Implement  
**Estimated Duration:** 3-4 weeks

---

## 🎯 Executive Summary

This document outlines the test-driven development (TDD) approach for implementing Phase 1 of librustdash - the foundational Rust primitives library for Dash blockchain data structures.

After exploring the Dash Core codebase (`../src/`), we have concrete implementation details for:
- Transaction serialization format (version/type encoding)
- All 10 transaction types
- BLS signature sizes (48-byte pubkey, 96-byte signature)
- Special transaction payload structures (CbTx, AssetLock, etc.)
- Test data locations for golden tests

### Key Principles

1. **Tests First, Always** - Write failing tests before implementation (Red-Green-Refactor)
2. **Byte-for-byte Compatibility** - Must match Dash Core serialization exactly
3. **Use Dash Core as Reference** - Source at `../src/` for all implementation details
4. **Golden Tests** - Real transaction/block data from Dash Core test files
5. **Property-Based Testing** - Random generation for edge cases
6. **No Premature Optimization** - Correctness first, speed later

### Research Summary

From Dash Core codebase exploration:

**Serialization Format** (`../src/primitives/transaction.h:244-250`):
```cpp
int32_t n32bitVersion = this->nVersion | (this->nType << 16);
s << n32bitVersion;
s << vin;
s << vout;
s << nLockTime;
if (this->HasExtraPayloadField())
    s << vExtraPayload;
```

**BLS Sizes** (`../src/bls/bls.h:36-39`):
```cpp
constexpr int BLS_CURVE_PUBKEY_SIZE{48};
constexpr int BLS_CURVE_SIG_SIZE{96};
```

**Test Data Available**:
- `../src/test/data/tx_valid.json` - Normal transactions
- `../test/functional/feature_dip*.py` - Special transaction tests
- Can extract real hex data for golden tests

---

## 📦 Dependencies (Cargo.toml)

```toml
[package]
name = "libdashrust"
version = "0.1.0"
edition = "2021"

[dependencies]
bitcoin = { version = "0.32", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
hex = "0.4"
byteorder = "1.5"
thiserror = "1.0"

[dev-dependencies]
proptest = "1.5"
serde_json = "1.0"
```

---

## 🗂️ Project Structure

```
src/
├── lib.rs                 # Re-exports
├── error.rs               # DashError type
├── serialize.rs           # CompactSize & serialization traits
├── types.rs               # Common types (uint160, BLS wrappers)
├── hash.rs                # Hashing utilities
├── tx_type.rs             # DashTxType enum
├── transaction.rs         # Transaction, TxIn, TxOut
├── block.rs               # Block, BlockHeader
└── payloads/
    ├── mod.rs
    ├── coinbase.rs        # CbTx
    └── asset_lock.rs      # AssetLock/AssetUnlock

tests/
├── serialize_tests.rs
├── tx_type_tests.rs
├── transaction_tests.rs
├── block_tests.rs
└── golden/
    ├── mod.rs
    └── fixtures.json

examples/
├── parse_transaction.rs
└── parse_block.rs
```

---

## 📅 Implementation Schedule

### Week 1: Foundation

#### Day 1-2: Setup & Serialization
- [ ] Update Cargo.toml
- [ ] Create project structure
- [ ] Implement error types (TDD)
- [ ] Implement CompactSize (TDD)
- [ ] Little-endian primitives

#### Day 3: DashTxType Enum
- [ ] Write enum tests
- [ ] Implement DashTxType
- [ ] Conversion traits

#### Day 4-5: Transaction Basics
- [ ] Version/type encoding tests
- [ ] Basic Transaction structure
- [ ] TxIn/TxOut types
- [ ] Golden test with real tx

### Week 2: Transactions & Blocks

#### Day 1-3: Transaction Complete
- [ ] Full Transaction serialization
- [ ] Multiple golden tests
- [ ] Property-based tests
- [ ] Hash computation

#### Day 4-5: Blocks
- [ ] BlockHeader (80 bytes)
- [ ] Block with transactions
- [ ] Golden tests

### Week 3: Special Transaction Payloads

#### Day 1-3: CbTx (Coinbase)
- [ ] CbTx v1 tests
- [ ] CbTx v2 tests (quorums)
- [ ] CbTx v3 tests (chainlock)
- [ ] Versioned serialization

#### Day 4-5: AssetLock/AssetUnlock
- [ ] AssetLockPayload tests
- [ ] AssetUnlockPayload tests
- [ ] BLS signature as opaque bytes

### Week 4: Polish & Documentation

- [ ] All examples working
- [ ] Documentation complete
- [ ] Test coverage > 80%
- [ ] README with usage
- [ ] CI setup

---

## 🧪 Test-Driven Development Workflow

### For EVERY Component:

1. **RED**: Write failing test
2. **GREEN**: Implement minimal code to pass
3. **REFACTOR**: Clean up code
4. **DOCUMENT**: Add rustdoc comments
5. **VERIFY**: Run all checks

### Test Requirements

Every component must have:
- ✅ Unit tests
- ✅ At least one golden test (real Dash data)
- ✅ Round-trip test (serialize → deserialize → serialize)
- ✅ Property-based test (if applicable)
- ✅ Error case tests

---

## 🎯 Success Criteria

Phase 1 is **DONE** when:

1. ✅ Can deserialize 100% of normal transactions
2. ✅ Can deserialize CbTx (v1, v2, v3) from Dash Core
3. ✅ Can deserialize AssetLock/AssetUnlock
4. ✅ Can deserialize block headers
5. ✅ All round-trip tests pass (byte-for-byte match)
6. ✅ Test coverage > 80%
7. ✅ Zero clippy warnings
8. ✅ Documentation complete
9. ✅ Examples work
10. ✅ README explains usage

---

## 📝 Key Implementation Details

### Version/Type Encoding
```rust
// Dash combines version and type in 32 bits:
// n32bitVersion = (nType << 16) | nVersion
let combined = ((tx_type as u32) << 16) | (version as u32 & 0xFFFF);
```

### BLS Signatures (Phase 1)
```rust
// Treat as opaque bytes for now
pub struct BlsSignature(pub Vec<u8>); // 96 bytes
pub struct BlsPublicKey(pub Vec<u8>); // 48 bytes
```

### CompactSize Format
```
0-252:          1 byte
253-65535:      0xFD + 2 bytes (little-endian)
65536-2^32-1:   0xFE + 4 bytes (little-endian)
2^32-2^64-1:    0xFF + 8 bytes (little-endian)
```

### Test Data Sources
1. `../src/test/data/tx_valid.json` - Bitcoin-style transactions
2. `../test/functional/` - Functional tests with real data
3. Extract from running dashd if available

---

## 🚀 Daily Checklist

```bash
# Run tests
cargo test

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy -- -D warnings

# Check docs build
cargo doc --no-deps

# Run examples
cargo run --example parse_transaction
```

---

## ✅ Definition of Done

A component is "done" when:

1. ✅ Tests written FIRST (TDD)
2. ✅ All tests pass
3. ✅ At least one golden test
4. ✅ Round-trip test passes
5. ✅ Rustdoc comments on all public items
6. ✅ Example code works
7. ✅ No compiler warnings
8. ✅ Coverage meets target
9. ✅ Code reviewed (self or peer)
10. ✅ Git committed with clear message

---

## 🔗 References

- Dash Core Source: `../src/`
- Transaction Header: `../src/primitives/transaction.h`
- Block Header: `../src/primitives/block.h`
- Special Tx: `../src/evo/specialtx.h`
- CbTx: `../src/evo/cbtx.h`
- AssetLock: `../src/evo/assetlocktx.h`
- BLS: `../src/bls/bls.h`
- Serialization: `../src/serialize.h`

---

**Let's build this! 🚀**
