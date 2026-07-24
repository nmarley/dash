# daino -- Project Status Report

**Updated:** July 24, 2026

## Overview

Daino is a modern Rust blockchain indexer for Dash, designed to replace
the aging Insight block explorer. It is modeled after Zaino (the Zcash
indexer): a read-only companion that maintains its own LMDB index and
serves a REST API. The long-term vision is to replace Dash Core's
client-serving role entirely, with a separate consensus validator
handling validation only.

## Current State

- **~40 tests passing** across the workspace (daino-core, daino-state,
  daino-fetch, dainod integration, doc-tests)
- **Compiles cleanly** with current deps (re-verified July 2026)
- **~5,900 lines of Rust** in the workspace (excluding target/)
- **Edition:** Rust 2024, resolver 3
- **Branch tip:** `r2`
- **Block and transaction responses** match Insight for core fields
  (vin/vout, spent-by, difficulty, reward, chainwork)
- **Active plan:** `PLAN-daino-foundation.md` (foundation before features)

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `daino-core` | Block/undo file reading, difficulty/chainwork math |
| `daino-state` | LMDB-backed storage (10 named databases) |
| `daino-fetch` | dashd JSON-RPC client, ZMQ subscriber (scaffold), follower |
| `daino-serve` | Axum REST API server (Insight-compatible) |
| `dainod` | CLI binary, two-pass indexer, integration tests |

## What's Implemented

### Core infrastructure

1. Five-crate Cargo workspace with clear separation of concerns
2. Two-pass chain-ordering indexer (header scan, chain walk, ordered read)
3. Read-ahead pipeline (reader thread + bounded channel + writer)
4. Resume/incremental indexing from existing tip
5. Undo (`rev*.dat`) offset scan and read during Pass 2
6. Input-side address indexing via undo spent outputs

### Storage (LMDB, 10 databases)

- blocks_by_height, hash_to_height
- txs_by_id, block_txs, tx_raw
- addr_to_txs, utxos, addr_utxos
- spent_by
- meta (chain tip and counts)
- Compaction command for dead page reclamation
- Slim UTXO value layout (non-key fields only)

### REST API (Insight-compatible)

DB-backed: status, block by hash, block-index by height, tx by txid,
addr summary, addr txs, addr utxo.

dashd-backed (optional RPC): chainlock, sporks, governance list.

### Block math

- difficulty_from_bits (Bitcoin difficulty-1 reference)
- work_from_bits and cumulative chainwork in BlockRecord
- reward from coinbase outputs at response time

### Optimizations (historical measurements)

- LMDB batched writes: large speedup vs per-block commits
- Slim UTXO + compaction: substantial DB size reduction
- Read-ahead pipeline: modest throughput gain over single-thread

## Gap Analysis

| Aspect | Status |
|--------|--------|
| Block response vs Insight | Match (core fields) |
| Tx response vin/vout/spent-by | Match (core fields) |
| Address balance / pagination | Not implemented (deferred plan) |
| Block disconnect / reorg | Not implemented |
| Shared apply path (index vs follow) | Not unified |
| TxProvider trait | Designed only |
| ZMQ live subscribe | Scaffold only; follow polls RPC |
| dashd cross-check tool | Not implemented |
| Real-time WebSocket events | Not implemented |

## Remaining Roadmap

### Near-term (foundation) -- PLAN-daino-foundation.md

1. Re-verify builds/tests and keep docs truthful (this report)
2. Single shared block-apply pipeline for index and follow
3. Disconnect/reorg support with tests
4. TxProvider trait in the serve path
5. Correctness gate against dashd RPC

### After foundation

- Pagination and balance (`docs/PLAN_PAGINATION_BALANCE.md`)
- ZMQ subscription, WebSocket events
- script asm, special tx extraPayload in API
- Dash-specific indexes (MN payments, governance, IS, ChainLock in-index)

## Key Design Decisions

- Build from scratch, not fork Zaino (architectural blueprint only)
- LMDB over RocksDB
- REST over gRPC for Insight compatibility first
- X11 via FFI (`~/projects/x11-hash`), librustdash `x11` feature
- Two-pass indexer because blk files are received-order
- Raw tx bytes in LMDB for Phase-1 self-contained serve
  (see VISION_RUST_VALIDATOR.md for future validator-backed TxProvider)

## Documentation

| Document | Description |
|----------|-------------|
| `daino/AGENTS.md` | Full agent context |
| `daino/README.md` | User-facing overview |
| `daino/docs/VISION_RUST_VALIDATOR.md` | Long-term architecture |
| `PLAN-daino-foundation.md` | Active foundation plan |
| `daino/docs/PLAN_CHAIN_ORDERING.md` | Two-pass design (done) |
| `daino/docs/PLAN_READ_AHEAD.md` | Pipeline design (done) |
| `daino/docs/PLAN_PAGINATION_BALANCE.md` | Deferred features |
| `librustdash/docs/DAINO_ARCHITECTURE.md` | Original full design |

## Branch Lineage

```
develop (Dash Core C++)
  +-- ngm-rust (librustdash primitives)
       +-- daino (indexer scaffolding)
            +-- reorgs (undo file reading)
                 +-- massive-refactor / wip-verify1 (LMDB, API, two-pass)
                      +-- r2 (tx parity, spent-by, undo in indexer) <-- tip
```

Development began Dec 31, 2025. Last feature commits on r2: March 2026.
Foundation resume: July 2026.
