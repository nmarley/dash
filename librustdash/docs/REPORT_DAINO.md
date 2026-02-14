# daino -- Project Status Report

**Date:** February 14, 2026
**Location:** `/Users/nathan/projects/dash/daino`
**Branch:** `reorgs` (clean)

## Overview

daino is a Dash blockchain block and undo file reader -- a CLI tool written in
Rust that parses Dash Core's raw data files (`blk*.dat` and `rev*.dat`). It is
Stage 1 of what is planned to become a full Dash blockchain indexer, a modern
replacement for the Insight block explorer that Dash has used for 10+ years.

The name and concept are modeled after Zaino, the Zcash blockchain indexer.

## Current State

- **2 tests passing** (amount decompression, varint decoding)
- **Compiles cleanly** with zero warnings
- **~850 lines of Rust** across 4 source files
- **Edition:** Rust 2024
- **Stage 1 of 4** -- proof-of-concept file reader

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `librustdash` | path | Block deserialization |
| `clap` | 4.5 | CLI argument parsing (derive) |
| `byteorder` | 1.5 | Endian-aware integer reading |
| `hex` | 0.4 | Hex encoding for display |
| `anyhow` | 1.0 | Error handling |
| `sha2` | 0.10 | SHA256 checksums |

No async runtime, no storage engine, no gRPC -- purely synchronous file reader.

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | 254 | CLI (clap), block/undo display, coinbase message extraction |
| `src/block_reader.rs` | 123 | `BlockFileReader` -- reads `blk*.dat`, magic byte validation |
| `src/undo.rs` | 348 | Undo data structures, varint/CompactSize, amount/script decompression |
| `src/undo_reader.rs` | 130 | `UndoFileReader` -- reads `rev*.dat`, optional SHA256 checksum |

## CLI Interface

```
Usage: daino [OPTIONS] <FILE>

Arguments:
  <FILE>  Path to the file (e.g., blk00000.dat or rev00000.dat)

Options:
  -n, --network <NETWORK>      Network type [default: mainnet]
  -c, --count <COUNT>          Number of blocks/undo records to read [default: 1]
  -u, --undo                   Read undo file (rev*.dat)
      --show-coinbase-message  Show coinbase message (genesis block)
      --show-raw-block         Show raw block header bytes (for PoW)
  -h, --help                   Print help
```

## What's Implemented (Stage 1)

1. Read and parse blocks from `blk*.dat` files sequentially
2. Read and parse undo data from `rev*.dat` files sequentially
3. Mainnet, testnet, and regtest network magic bytes
4. Block header field display (version, prev hash, merkle root, time, bits, nonce)
5. Coinbase transaction details (version, type, I/O counts, extra payload)
6. Coinbase message extraction (parses scriptSig opcodes for ASCII)
7. Raw 80-byte block header hex display (for PoW verification)
8. Spent UTXO details from undo data (value, scriptPubKey, height, coinbase flag)
9. Compressed script decompression (P2PKH, P2SH, P2PK compressed/uncompressed, raw)
10. Bitcoin VarInt (7-bit continuation) and CompactSize encoding
11. Amount decompression algorithm (matching Dash Core `compressor.h`)

## Test Data

In `data/` (.gitignored):
- `blk00000.dat`, `blk00001.dat` -- Real Dash mainnet block files
- `rev00000.dat`, `rev00001.dat` -- Real Dash mainnet undo files

## Gap Analysis: Current vs. Planned Architecture

The architecture docs (`librustdash/docs/DAINO_ARCHITECTURE.md` and
`INDEXER_ARCHITECTURE.md`) describe a much larger system:

| Aspect | Current | Planned |
|--------|---------|---------|
| Structure | Single flat binary | 9-crate workspace |
| Block reading | blk*.dat sequential | + RPC, ZMQ backends |
| Storage | None (display only) | LMDB or RocksDB |
| Indexing | None | Block, address, masternode, governance |
| API | CLI only | gRPC server (tonic) |
| Async | Synchronous | tokio runtime |
| Tests | 2 unit tests | Unit, integration, golden, property-based |
| Error handling | anyhow | thiserror + anyhow |
| Real-time sync | None | ZMQ subscriber |

## Planned Architecture (from design docs)

### Workspace Crates
1. `daino-common` -- shared types and traits
2. `daino-proto` -- protobuf/gRPC definitions
3. `daino-fetch` -- block source backends (file, RPC, ZMQ)
4. `daino-state` -- storage and indexing (LMDB/RocksDB)
5. `daino-serve` -- gRPC server
6. `dainod` -- main daemon binary
7. `daino-testutils` -- test helpers
8. `daino-testvectors` -- test data
9. `integration-tests` -- end-to-end tests

### Planned Indexing Features (Insight Replacement)

- Block index (hash -> file position, height)
- Transaction index (txid -> block, position)
- Address index (address -> transaction history)
- UTXO set tracking
- Masternode list tracking
- ChainLock tracking
- InstantSend lock tracking
- Governance object indexing
- Credit pool / Platform asset tracking

### Implementation Roadmap (from DAINO_ARCHITECTURE.md)

- Phase 1: Foundation (workspace, common types, test infra)
- Phase 2: Block fetching (file reader, dashd RPC)
- Phase 3: Storage (LMDB/RocksDB, block/tx indexing)
- Phase 4: Address indexing
- Phase 5: Masternode indexing
- Phase 6: gRPC API
- Phase 7: Real-time sync (ZMQ)
- Phase 8: Advanced features (ChainLocks, governance, Platform)
- Phase 9: Production hardening

## Insight API Surface (for replacement target)

The current Insight block explorer provides these endpoint categories:

- **Blocks**: by hash, by height, by date, raw hex
- **Transactions**: by txid, by block, by address, raw hex, broadcast (+ IS)
- **Addresses**: summary, balance, totalReceived/Sent, unconfirmed
- **UTXOs**: by address, multi-address
- **Dash-specific**: sporks, governance (proposals/triggers/votes/budgets),
  masternodes (deprecated), ChainLocks, InstantSend (txlock field on all txs)
- **Network**: status, sync, peers, fee estimation
- **Real-time**: socket.io events for new tx, txlock, blocks, address activity

## Key Design Decisions

- **Build from scratch, not fork Zaino** -- 70-80% of Zaino is Zcash-specific;
  use as architectural blueprint only
- **LMDB preferred over RocksDB** -- simpler, crash-safe, good read performance;
  RocksDB as alternative if write-heavy workload demands it
- **Trait-based abstractions** -- `BlockSource`, `Storage`, `BlockProcessor`
  traits allow swappable backends
- **X11 not needed initially** -- indexer doesn't validate PoW, just indexes

## Git History

Development: Dec 31, 2025 through Jan 15, 2026 (~28 commits on daino/reorgs
branches). The `reorgs` branch extends `daino` with undo file reading support.
Last commit: Jan 15, 2026 (cleanup/declutter tool output).
