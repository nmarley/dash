# librustdash -- Project Status Report

**Date:** February 14, 2026
**Location:** `/Users/nathan/projects/dash/librustdash`
**Branch:** `reorgs` (clean, up to date with `nmarley/reorgs`)

## Overview

librustdash is a Rust primitives library for Dash blockchain data structures.
It provides types and serialization for blocks, transactions, and special
transaction payloads, with byte-for-byte compatibility with Dash Core. It is
modeled after `librustzcash` in the Zcash ecosystem.

## Current State

- **105 tests passing** (98 unit + 2 golden + 5 doc-tests)
- **Phase 1 complete** -- all primitives implemented
- **Working tree clean** -- no uncommitted changes
- **Edition:** Rust 2021

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `bitcoin` | 0.32 | Bitcoin-compatible types (with serde) |
| `serde` | 1.0 | Serialization framework |
| `hex` | 0.4 | Hex encoding/decoding |
| `byteorder` | 1.5 | Endian-aware integer I/O |
| `thiserror` | 1.0 | Error derive macros |
| `sha2` | 0.10 | SHA-256 hashing |

Dev: `proptest 1.5`, `serde_json 1.0`

## Source Modules

| File | Purpose |
|------|---------|
| `lib.rs` | Crate root, public re-exports |
| `block.rs` | BlockHeader (80 bytes) and Block types |
| `transaction.rs` | Transaction, TxIn, TxOut, OutPoint with version/type encoding |
| `tx_type.rs` | DashTxType enum (all 10 Dash transaction types) |
| `serialize.rs` | CompactSize varint read/write |
| `payloads/` | All 9 special transaction payload types |
| `bls.rs` | BLS signature types (opaque bytes) |
| `bitvector.rs` | Bit vector utilities |
| `network_info.rs` | Network parameters |
| `error.rs` | Error types |

### Special Transaction Payloads (in `src/payloads/`)

All 9 payload types implemented with serialization and doc-tests:

- CbTx (Coinbase, v1/v2/v3)
- AssetLockPayload
- AssetUnlockPayload
- ProRegTx (Masternode registration)
- ProUpServTx (Masternode service update)
- ProUpRegTx (Masternode registrar update)
- ProUpRevTx (Masternode revocation)
- QuorumCommitment (LLMQ quorum commitment)
- MnhfSignal (Masternode hard fork signal)

## Test Coverage

- 3 serialization tests (CompactSize encoding/decoding, edge cases)
- 4 transaction type tests (enum conversions, display)
- 11 transaction tests (version/type encoding, empty tx, coinbase, special tx)
- 6 block tests (header size, roundtrip, blocks with transactions)
- 2 golden tests (real Dash mainnet transaction roundtrip, version/type decoding)
- 5 doc-tests (ProRegTx, ProUpServTx, ProUpRegTx, ProUpRevTx, MnhfSignal)
- Plus additional tests added since the Phase 1 report (total now 105)

## Branch Lineage

```
develop (Dash Core C++)
  +-- ngm-rust (librustdash primitives)
       +-- daino (indexer scaffolding)
            +-- reorgs (undo file reading) <-- current
```

## What's Implemented (Phase 1 -- COMPLETE)

- Error handling with thiserror
- CompactSize varint serialization
- DashTxType enum with all 10 variants
- Transaction, TxIn, TxOut, OutPoint with Dash version/type encoding
- BlockHeader (80-byte) and Block types
- CbTx payload (v1, v2, v3)
- AssetLock/AssetUnlock payloads
- ProRegTx, ProUpServTx, ProUpRegTx, ProUpRevTx payloads
- QuorumCommitment, MnhfSignal payloads
- BLS types, BitVector, network info
- Golden tests with real Dash mainnet data

## Planned Next Steps

1. **Hash utilities** (sha256d, txid, block hash, merkle root) -- detailed
   plan in `docs/HASH_INTEGRATION_PLAN.md`
2. **dash_protocol crate** -- network constants, activation heights
3. **dash_consensus crate** -- X11 PoW (via FFI), difficulty adjustment,
   block/tx validation (long-term)

## Design Documents

All in `docs/`:

| Document | Lines | Description |
|----------|-------|-------------|
| ARCHITECTURE.md | 390 | Crate organization (librustzcash pattern) |
| DAINO_ARCHITECTURE.md | 1465 | Full indexer architecture design |
| HASH_INTEGRATION_PLAN.md | 683 | Hash utilities implementation plan |
| IMPLEMENTATION_PLAN.md | 292 | Phase 1 TDD plan (completed) |
| INDEXER_ARCHITECTURE.md | 1136 | Indexer design with trait abstractions |
| LIBRUSTDASH_DESIGN.md | 817 | Core design document |
| ROADMAP.md | 378 | 5-phase modernization roadmap |
| SERIALIZATION.md | 634 | Binary format reference |
| WHERE_TO_ORGANIZE_X11.md | 513 | X11 architectural decision |
| X11_RUST_PORT_ANALYSIS.md | 577 | X11 feasibility analysis |
| X11_SUMMARY.md | 104 | X11 decision summary |

## Downstream Consumers

- **daino** (`/Users/nathan/projects/dash/daino`) -- uses `Block::deserialize()`
- **dmtv3** (`/Users/nathan/projects/dmtv3`) -- uses Transaction, TxIn, TxOut,
  OutPoint, DashTxType, ProRegTx, ProUpServTx, Block, BlockHeader
