# daino -- Project Status Report

**Updated:** February 14, 2026

## Overview

Daino is a modern Rust blockchain indexer for Dash, designed to replace
the aging Insight block explorer. It is modeled after Zaino (the Zcash
indexer): a read-only companion that maintains its own LMDB index and
serves a REST API. The long-term vision is to replace Dash Core's
client-serving role entirely, with a separate consensus validator
handling validation only.

The name and concept are modeled after Zaino, the Zcash blockchain
indexer.

## Current State

- **37 tests passing** (19 daino-core + 6 daino-state + 6 daino-fetch
  + 5 integration + 1 doc-test)
- **Compiles cleanly** with zero warnings
- **~5,100 lines of Rust** across 25 source files in 5 crates
- **Edition:** Rust 2024, resolver 3
- **Block response fully matches Insight** (all fields verified
  side-by-side)
- **27 daino-specific commits**

## Workspace Crates

| Crate | Lines | Purpose |
|-------|-------|---------|
| `daino-core` | ~1,200 | Block/undo file reading, difficulty/chainwork math |
| `daino-state` | ~900 | LMDB-backed storage (8 databases, slim UTXO) |
| `daino-fetch` | ~720 | dashd JSON-RPC client, ZMQ subscriber (scaffold), follower |
| `daino-serve` | ~750 | Axum REST API server (Insight-compatible) |
| `dainod` | ~1,500 | CLI binary, two-pass indexer, integration tests |

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `librustdash` | path (x11 feature) | Block deserialization, hashing, script analysis |
| `heed` | latest | Safe LMDB wrapper |
| `axum` | 0.8 | HTTP framework for REST API |
| `tokio` | 1.x | Async runtime |
| `clap` | 4.5 | CLI argument parsing (derive) |
| `reqwest` | 0.12 | HTTP client for dashd JSON-RPC |
| `bincode` | latest | Compact binary serialization for DB values |
| `serde` | 1.0 | Serialization framework |
| `anyhow` | 1.0 | Error handling |
| `hex` | 0.4 | Hex encoding |
| `tower-http` | 0.6 | CORS middleware |

## CLI Interface

```
Usage: dainod <COMMAND>

Commands:
  read      Read raw block/undo files and display contents
  index     Index block files into LMDB (two-pass, chain-ordered)
  status    Show database status and file size
  serve     Start REST API server
  follow    Follow dashd via RPC and continuously index new blocks
  compact   Compact the database (reclaim dead pages)

Index options:
  --datadir <PATH>       Directory containing blk*.dat files
  --dbdir <PATH>         LMDB database directory
  --network <NETWORK>    Network type [default: mainnet]
  --batch-size <N>       Blocks per LMDB transaction [default: 1000]
  --max-blocks <N>       Maximum blocks to index (0 = all) [default: 0]
  --compact              Compact database after indexing

Serve options:
  --dbdir <PATH>         LMDB database directory
  --port <PORT>          Listen port [default: 3141]
  --rpc-url <URL>        dashd RPC URL (optional, for live queries)
  --rpc-user <USER>      dashd RPC username
  --rpc-password <PASS>  dashd RPC password
```

## What's Implemented

### Core Infrastructure
1. **5-crate Cargo workspace** with clear separation of concerns
2. **Two-pass chain-ordering indexer:** Pass 1 scans all headers via
   `scan_headers()` (80 bytes + seek per block), builds chain via
   forward-link walk from genesis (longest-chain rule handles forks),
   Pass 2 reads full blocks in chain order via `read_block_at()`
3. **Read-ahead pipeline:** Reader thread (disk I/O + deserialization +
   X11/SHA-256d hashing) sends through bounded `sync_channel(500)` to
   writer thread (script analysis + LMDB writes)
4. **Resume/incremental indexing:** Detects existing tip, continues
   from next block

### Storage (LMDB, 8 databases)
5. Block index (by height and by hash)
6. Transaction index (by txid)
7. Block-to-txids mapping (height -> concatenated raw txids)
8. Address history index (prefix-scannable compound keys)
9. UTXO set with slim storage (~33 bytes per entry)
10. Address UTXO index (prefix-scannable)
11. Chain metadata (tip, counts)
12. Database compaction (dead page reclamation)

### REST API (Insight-compatible)
13. `GET /api/status` -- chain info
14. `GET /api/block/:hash` -- full block with difficulty, reward,
    chainwork, tx array, confirmations, prev/next hash, chainlock
15. `GET /api/block-index/:height` -- height to hash
16. `GET /api/tx/:txid` -- transaction summary
17. `GET /api/addr/:addr` -- address summary
18. `GET /api/addr/:addr/txs` -- address transaction history
19. `GET /api/addr/:addr/utxo` -- address UTXOs
20. `GET /api/chainlock` -- best ChainLock (dashd-backed)
21. `GET /api/sporks` -- active sporks (dashd-backed)
22. `GET /api/governance/list` -- governance proposals (dashd-backed)

### Block Math
23. `difficulty_from_bits()` -- Bitcoin difficulty-1 reference
    (`0x1d00ffff`), matches Insight exactly
24. `work_from_bits()` -- proof-of-work per block (`2^256 / (target+1)`)
25. `chainwork` -- cumulative 256-bit accumulator, stored in BlockRecord
26. `reward` -- coinbase output value, queried at response time

### Optimizations
27. LMDB batched writes (95x speedup: 187 -> 17,797 blk/s)
28. Slim UTXO storage (48.2% DB size reduction: 806 MB -> 417 MB)
29. Read-ahead pipeline (~14% throughput: 3,288 -> 3,734 blk/s)

## Test Data

In `data/` (.gitignored):
- `blk00000.dat`, `blk00001.dat`, `blk00002.dat` -- Real Dash mainnet
  block files
- `rev00000.dat`, `rev00001.dat` -- Real Dash mainnet undo files

Integration tests read blocks from `data/blk00000.dat`, index them into
a temp LMDB database using `put_block()`, spin up the API server on a
random port, and verify HTTP responses.

## Gap Analysis: Current vs. Insight

| Aspect | Current | Insight | Status |
|--------|---------|---------|--------|
| Block response | All fields | All fields | MATCH |
| Tx response | Summary only | Full vin/vout | GAP |
| Tx vin array | Not stored | scriptSig, sequence, address | GAP |
| Tx vout array | Not stored | scriptPubKey hex/asm/addresses | GAP |
| Spent info | Not tracked | spentTxId/spentIndex/spentHeight | GAP |
| Tx size | Not stored | Byte size | GAP |
| isCoinBase | Not stored | Boolean flag | GAP |
| Address balance | Not implemented | balance endpoint | GAP |
| Pagination | Not implemented | from/to params | GAP |
| Real-time events | Not implemented | socket.io | GAP |
| Masternode index | Not implemented | (deprecated in Insight) | N/A |

## Remaining Roadmap

### Near-term (tx parity with Insight)
- Store vin/vout detail per transaction (scriptPubKey, addresses, values)
- Add spending-tx index (which tx spent each output)
- Add `blockhash`, `blocktime`, `size`, `isCoinBase` to tx response
- Return `valueOut` as integer (satoshis), not float

### Medium-term
- Pagination for address tx/utxo endpoints
- Balance endpoint (`/api/addr/{addr}/balance`)
- Auto-reload DB in serve mode when data file changes
- WebSocket / real-time event notifications

### Long-term (from original architecture docs)
- ZMQ subscriber for real-time block/tx notifications
- ChainLock tracking in index (not just dashd passthrough)
- InstantSend lock tracking
- Governance object indexing
- Reorg handling
- Production hardening

## Key Design Decisions

- **Build from scratch, not fork Zaino** -- 70-80% of Zaino is
  Zcash-specific; use as architectural blueprint only
- **LMDB over RocksDB** -- simpler, crash-safe, excellent read
  performance, single-writer design fits our pipeline model
- **REST over gRPC** -- Insight compatibility is the priority; gRPC
  can be added later as an additional interface
- **X11 via FFI** -- C library at `~/projects/x11-hash`, behind
  librustdash `x11` feature flag. Fast enough that it's not a
  bottleneck even in the read-ahead pipeline.
- **Two-pass indexer** -- necessary because block files contain blocks
  in received order, not chain order. Pass 1 (header scan) is fast;
  repeating it on resume is acceptable.

## Documentation

### In daino repo (`/Users/nathan/projects/dash/daino/docs/`)

| Document | Description |
|----------|-------------|
| `AGENT.md` | Full agent context (architecture, schema, API, conventions) |
| `IDEAS.md` | Backlog / low-priority improvements |
| `PLAN_CHAIN_ORDERING.md` | Two-pass indexer design (implemented) |
| `PLAN_READ_AHEAD.md` | Reader/writer pipeline design (implemented) |
| `DASH_DATA_DIRECTORY.md` | Dash Core data file formats and layout |

### In librustdash repo (`docs/`)

| Document | Description |
|----------|-------------|
| `DAINO_ARCHITECTURE.md` | Original full indexer architecture design |
| `INDEXER_ARCHITECTURE.md` | Indexer design with trait abstractions |

## Git History

Development began Dec 31, 2025. 27 daino-specific commits on
daino/reorgs branches. Branch lineage: `develop` -> `ngm-rust` ->
`daino` -> `reorgs`.

Key milestones:
- Dec 31, 2025: Initial commit, block file reader
- Jan 2026: Undo file reading, workspace refactor
- Feb 14, 2026: Full LMDB indexing, REST API, Insight block parity
- Feb 15, 2026: difficulty/reward/chainwork, exact Insight alignment
