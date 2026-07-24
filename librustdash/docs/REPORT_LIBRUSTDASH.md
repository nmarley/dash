# librustdash -- Project Status Report

**Date:** July 24, 2026
**Location:** `/Users/nathan/projects/dashpay/dash/librustdash`
**Branch:** `r2` (with daino; library also used from earlier lineage)

## Overview

librustdash is a Rust primitives library for Dash blockchain data structures.
It provides types and serialization for blocks, transactions, and special
transaction payloads, with byte-for-byte compatibility with Dash Core. It is
modeled after `librustzcash` in the Zcash ecosystem.

## Current State

- **120 tests passing** (113 unit + 2 golden + 5 doc-tests)
- **Phase 1 complete** -- primitives, payloads, hashing, basic scripts
- **Edition:** Rust 2021
- **x11 path:** `../../../x11-hash/rust` relative to this crate
  (repo lives at `projects/dashpay/dash`; x11-hash at `projects/x11-hash`)
- **Re-verified:** July 2026 (`cargo test`, `make libx11.a`)

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `bitcoin` | 0.32 | Bitcoin-compatible types (with serde) |
| `serde` | 1.0 | Serialization framework |
| `hex` | 0.4 | Hex encoding/decoding |
| `byteorder` | 1.5 | Endian-aware integer I/O |
| `thiserror` | 1.0 | Error derive macros |
| `sha2` | 0.10 | SHA-256 hashing |
| `x11_hash` | path (optional feature) | X11 PoW hash via C FFI |

Dev: `proptest`, `serde_json`, `x11_hash` (always on for tests)

## Source Modules

| File | Purpose |
|------|---------|
| `lib.rs` | Crate root, public re-exports |
| `block.rs` | BlockHeader (80 bytes) and Block types |
| `transaction.rs` | Transaction, TxIn, TxOut, OutPoint |
| `tx_type.rs` | DashTxType enum (all 10 types) |
| `serialize.rs` | CompactSize varint read/write |
| `hash.rs` | sha256d, x11_hash, display helpers |
| `script.rs` | Script analysis, address encode/decode |
| `payloads/` | All 9 special transaction payload types |
| `bls.rs` | BLS signature types (opaque bytes) |
| `bitvector.rs` | Bit vector utilities |
| `network_info.rs` | ServiceAddress and related |
| `error.rs` | Error types |

### Special Transaction Payloads

All 9 implemented with serialization and tests:

- CbTx (Coinbase, v1/v2/v3)
- AssetLockPayload / AssetUnlockPayload
- ProRegTx, ProUpServTx, ProUpRegTx, ProUpRevTx
- QuorumCommitment, MnhfSignal

## What's Implemented (Phase 1 -- COMPLETE)

- Error handling with thiserror
- CompactSize varint serialization
- DashTxType enum with all 10 variants
- Transaction, TxIn, TxOut, OutPoint with Dash version/type encoding
- BlockHeader and Block types
- All special tx payloads listed above
- BLS opaque types, BitVector, network address helpers
- SHA-256d and optional X11 hashing
- Script type analysis and Dash address encode/decode
- Golden tests with real Dash mainnet data

## Planned Next Steps (library)

These remain library-side; daino foundation work is separate
(`PLAN-daino-foundation.md`):

1. Stronger merkle root helpers / more hash golden vectors if needed
2. Deeper script support as consumers require it
3. Longer-term: consensus-oriented crates (validation, not Phase 1)

Do not block daino foundation on a multi-crate workspace split.
Keep librustdash the single primitives home for now.

## Design Documents

All in `docs/`:

| Document | Description |
|----------|-------------|
| ARCHITECTURE.md | Crate organization (librustzcash pattern) |
| DAINO_ARCHITECTURE.md | Full indexer architecture design |
| HASH_INTEGRATION_PLAN.md | Hash utilities plan (largely done in tree) |
| IMPLEMENTATION_PLAN.md | Phase 1 TDD plan (completed) |
| INDEXER_ARCHITECTURE.md | Indexer design with trait abstractions |
| LIBRUSTDASH_DESIGN.md | Core design document |
| ROADMAP.md | Multi-phase modernization roadmap |
| SERIALIZATION.md | Binary format reference |
| X11_* | X11 port analysis and decisions |
| REPORT_DAINO.md | daino status |
| REPORT_DMTV3.md | dmtv3 status |

## Downstream Consumers

- **daino** (`../daino`) -- block deserialize, X11 hash, script/address
- **dmtv3** (external) -- Transaction types and ProTx payloads when wired

## Branch Lineage

```
develop (Dash Core C++)
  +-- ngm-rust (librustdash primitives)
       +-- daino / reorgs / massive-refactor / r2
```
