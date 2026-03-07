# Daino -- Agent Context

## Overview

Daino is a modern Rust blockchain indexer for Dash, designed to replace
the aging Insight block explorer. It is modeled after Zaino (the Zcash
indexer): a read-only companion that maintains its own LMDB index and
serves a REST API. The long-term vision is to replace Dash Core's
client-serving role entirely (like Zaino replaces Zcashd), with a
separate consensus validator handling validation only.

This is greenfield code. No backward compatibility needed -- blow away
the DB and re-index anytime.

## Related Documentation

Read these for deeper context as needed:

- `docs/IDEAS.md` -- backlog / low-priority improvements
- `docs/PLAN_CHAIN_ORDERING.md` -- two-pass indexer design (implemented)
- `docs/PLAN_READ_AHEAD.md` -- reader/writer pipeline design (implemented)
- `docs/DASH_DATA_DIRECTORY.md` -- Dash Core data file formats and layout
- `docs/VISION_RUST_VALIDATOR.md` -- long-term architecture: Rust validator + Zaino-style Daino
- `../librustdash/docs/REPORT_DAINO.md` -- daino project status report
- `../librustdash/docs/REPORT_LIBRUSTDASH.md` -- librustdash project status
- `../librustdash/docs/REPORT_DMTV3.md` -- dmtv3 project status

## Development Rules

- **Tests must be fast.** Do NOT open real `blk*.dat` files in new
  automated tests. Use synthetic data with `put_block()` for
  unit/integration tests. Tests that existed before and use real data
  are fine; do not add new ones that scan large files.
- **Best-in-class efficiency.** No shortcuts, canonical best practices
  for storage design. No redundant data, separate indexes over embedded
  data.
- **Schema changes require re-index.** If you add a field to
  `BlockRecord` or any stored type, the DB must be deleted and rebuilt.
- **Do NOT start dashd or dash-qt** for testing.
- **`--datadir` CLI arg** points directly at the directory containing
  `blk*.dat` files (no `blocks/` subdirectory assumption).
- Follow the commit conventions in `~/.config/Claude/AGENTS.md`.

## Build and Test

```bash
# Build everything
cargo build --workspace

# Run all tests (37 tests across workspace)
cargo test --workspace

# Run tests for a specific crate
cargo test -p daino-core
cargo test -p daino-state
cargo test -p daino-serve        # no unit tests, just compiles
cargo test -p dainod             # integration tests (needs data/blk00000.dat)

# Run a specific test
cargo test -p daino-core difficulty

# Run the indexer (example)
cargo run -p dainod -- index --datadir ~/path/to/blkfiles --dbdir /tmp/daino-db

# Run the API server
cargo run -p dainod -- serve --dbdir /tmp/daino-db --port 3141

# Check DB status
cargo run -p dainod -- status --dbdir /tmp/daino-db
```

## Technical Discoveries

These are non-obvious facts learned during development:

- **Dash uses X11 for block hashes, SHA-256d for txids.** librustdash's
  `BlockHeader::block_hash()` uses X11, `Transaction::txid()` uses
  SHA-256d.
- **X11 FFI:** The x11-hash C library lives at `~/projects/x11-hash`.
  `libx11.a` must be pre-built with `make libx11.a`. librustdash's X11
  support is behind an optional `x11` feature flag.
- **Dash address version bytes:** P2PKH mainnet=0x4c ('X'),
  P2SH mainnet=0x10 ('7'), P2PKH testnet=0x8c ('y'),
  P2SH testnet=0x13 ('8').
- **Block files contain blocks in received order, not chain order.**
  The two-pass indexer (Pass 1: scan headers, chain walk, Pass 2: read
  in chain order) solves this.
- **librustdash field names:** `TxIn.previous_output` (not
  `prev_output`), `OutPoint.hash` and `OutPoint.n` (not `.index`).
- **Difficulty convention:** All chains (including Dash) define
  difficulty relative to Bitcoin's difficulty-1 target (`bits =
  0x1d00ffff`). Dash genesis difficulty is ~0.000244140625 (1/4096).
- **LMDB memory-mapped behavior:** Even if you delete `data.mdb` on
  disk, a running `dainod serve` process still serves stale data from
  mapped pages. Must restart the server process.
- **heed API:** `env.copy_to_file(path, CompactionOption::Enabled)` for
  compaction, `env.real_disk_size()` for file size.
- **Overflow bug (fixed):** `start_height + usize::MAX as u32` wraps
  when `max_blocks=0` with nonzero `start_height`. Fixed with
  `saturating_add` / direct chain length.

## Workspace Structure

Five crates plus the `dainod` binary. ~5,100 lines of Rust, 37 tests.

### daino-core (`crates/daino-core/`)

Shared types and block/undo file reading.

| File | Lines | Purpose |
|------|-------|---------|
| `src/block_reader.rs` | 263 | `BlockFileReader` with `read_next_block()`, `scan_headers()`, `read_block_at()` |
| `src/difficulty.rs` | 449 | `target_from_bits()`, `difficulty_from_bits()`, `work_from_bits()`, `add_u256()`, `u256_to_hex()` |
| `src/undo.rs` | 347 | Undo data structures, varint, amount/script decompression |
| `src/undo_reader.rs` | 129 | `UndoFileReader` for `rev*.dat` files |
| `src/lib.rs` | 18 | Module declarations, re-exports |

Key types:
- `BlockFileReader` -- reads `blk*.dat`. `scan_headers()` for header-only pass (80 bytes + seek), `read_block_at(offset)` for seek-based random access.
- `ScannedHeader` -- `block_hash`, `prev_hash`, `time`, `file_offset`, `block_size`
- `Network` -- enum with magic bytes (Mainnet, Testnet, Regtest)

### daino-state (`crates/daino-state/`)

LMDB-backed storage. 8 named databases.

| File | Lines | Purpose |
|------|-------|---------|
| `src/db.rs` | 890 | `DainoDB` with all DB operations |
| `src/lib.rs` | 4 | Re-exports |

Key types:
- `BlockRecord` -- `height`, `hash`, `prev_hash`, `merkle_root`, `version: i32`, `time`, `bits`, `nonce`, `tx_count`, `size`, `chainwork: [u8; 32]`
- `TxRecord` -- `txid`, `block_height`, `tx_index`, `version: i16`, `tx_type: u16`, `lock_time`, `value_out: i64`, `input_count`, `output_count`
- `UtxoEntry` -- `txid`, `vout`, `value: i64`, `block_height`, `addr_hash: Option<[u8; 20]>`
- `AddrTxRef` -- `block_height`, `txid`
- `SpentOutpoint` -- `txid`, `vout`
- `BlockBatch` -- batch of block data for `put_batch()`
- `ChainMeta` -- `tip_height`, `tip_hash`, `block_count`, `tx_count`

DB methods: `put_block()`, `put_batch()`, `get_block_by_height()`,
`get_block_by_hash()`, `get_block_txids()`, `get_tx()`,
`get_addr_txs()`, `get_addr_utxos()`, `get_utxo()`, `get_meta()`,
`tip_height()`, `status_summary()`, `real_disk_size()`, `compact()`

### daino-fetch (`crates/daino-fetch/`)

dashd JSON-RPC client and ZMQ subscriber (scaffolded).

| File | Lines | Purpose |
|------|-------|---------|
| `src/rpc.rs` | 317 | `DashdRpc` (getblockchaininfo, getblock, getblockhash, getrawtransaction, getbestchainlock, getblockcount, call_raw) |
| `src/zmq.rs` | 237 | `ZmqSubscriber` (scaffolded, not fully wired) |
| `src/follower.rs` | 158 | `catch_up()`, `poll_loop()`, `FetchedBlock`, `FollowerConfig` |

### daino-serve (`crates/daino-serve/`)

Axum REST API server.

| File | Lines | Purpose |
|------|-------|---------|
| `src/api.rs` | 662 | All REST handlers and response types |
| `src/server.rs` | 81 | Router setup, `start_server()` |
| `src/lib.rs` | 10 | Re-exports |

### dainod (`dainod/`)

CLI binary with subcommands.

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | 210 | clap CLI: `read`, `index`, `status`, `serve`, `follow`, `compact` |
| `src/commands/index.rs` | 504 | Two-pass indexer with read-ahead pipeline |
| `src/commands/follow.rs` | 194 | dashd RPC tip-following |
| `src/commands/read.rs` | 188 | Raw block/undo file display |
| `src/commands/serve.rs` | 17 | Starts REST API server |
| `src/commands/status.rs` | 54 | Show DB status with file size |
| `src/commands/compact.rs` | 69 | Compaction with before/after sizes |
| `tests/api_integration.rs` | 310 | 5 integration tests (index real blocks, spin up API, verify HTTP) |

## LMDB Schema (10 databases)

| Database | Key | Value | Purpose |
|----------|-----|-------|---------|
| `blocks_by_height` | height (4B BE) | BlockRecord (bincode) | Block lookup by height |
| `hash_to_height` | block_hash (32B) | height (4B BE) | Block lookup by hash |
| `txs_by_id` | txid (32B) | TxRecord (bincode) | Tx lookup by txid |
| `block_txs` | height (4B BE) | N x 32B raw txids | Txid list for a block |
| `addr_to_txs` | addr(20)+height(4)+txid(32) | empty | Address tx history (prefix scan) |
| `utxos` | txid(32)+vout(4 BE) | UtxoValue (bincode) | UTXO lookup by outpoint |
| `addr_utxos` | addr(20)+txid(32)+vout(4 BE) | empty | Address UTXO index (prefix scan) |
| `meta` | "chain" (str) | ChainMeta (bincode) | Chain tip, counts |
| `tx_raw` | txid (32B) | raw serialized tx bytes | Full tx reconstruction at query time |
| `spent_by` | txid(32)+vout(4 BE) | spending_txid(32)+vin(4 BE)+height(4 BE) | Spent-by tracking for vout responses |

MAX_DBS=12 (room for future databases). MAX_DB_SIZE=20 GB.

Slim UTXO storage: private `UtxoValue` struct stores only non-key
fields (~33 bytes vs ~75 bytes per UTXO). The outpoint key already
encodes txid + vout.

## REST API Endpoints

### DB-backed (always available)

| Endpoint | Response | Notes |
|----------|----------|-------|
| `GET /api/status` | `{ info: { blocks, bestblockhash, txcount, version } }` | |
| `GET /api/block/:hash` | Full block with `difficulty`, `reward`, `chainwork`, `tx[]`, `confirmations`, `previousblockhash`, `nextblockhash`, `chainlock` | Matches Insight format |
| `GET /api/block-index/:h` | `{ blockHash }` | Height to hash |
| `GET /api/tx/:txid` | Full Insight tx: vin[], vout[], blockhash, blocktime, confirmations, size, valueIn, fees, isCoinBase | Matches Insight format |
| `GET /api/addr/:addr` | `{ addrStr, txCount }` | |
| `GET /api/addr/:addr/txs` | `{ addrStr, txCount, txids[] }` | Deduplicated |
| `GET /api/addr/:addr/utxo` | `[{ txid, vout, value, satoshis, height }]` | |

### dashd-backed (require `--rpc-*` flags)

| Endpoint | Notes |
|----------|-------|
| `GET /api/chainlock` | Best ChainLock from dashd |
| `GET /api/sporks` | Active sporks from dashd |
| `GET /api/governance/list` | Governance proposals from dashd |

## Insight Parity Status

### Block response -- FULLY ALIGNED

All fields match Insight exactly: `hash`, `size`, `height`, `version`,
`merkleroot`, `tx[]`, `time`, `nonce`, `bits`, `difficulty`, `reward`,
`chainwork`, `confirmations`, `previousblockhash`, `nextblockhash`.

Fields we don't emit yet (Insight-specific, low priority):
- `isMainChain` -- always true for us (no reorg support yet)
- `poolInfo` -- cosmetic (coinbase signature matching)

### Transaction response -- FULLY ALIGNED

All core Insight fields are present: `txid`, `version`, `type`,
`locktime`, `vin[]`, `vout[]`, `blockhash`, `blockheight`,
`confirmations`, `time`, `blocktime`, `isCoinBase`, `valueOut`,
`size`, `valueIn`, `fees`.

Each `vin` entry includes: `txid`, `vout`, `sequence`, `n`,
`scriptSig.hex`, `addr`, `valueSat`, `value`, `doubleSpentTxID`.
Coinbase inputs use: `coinbase`, `sequence`, `n`.

Each `vout` entry includes: `value` (8-decimal string), `n`,
`scriptPubKey.hex`, `scriptPubKey.addresses`, `scriptPubKey.type`,
`spentTxId`, `spentIndex`, `spentHeight`.

Input address/value resolution uses raw tx bytes from `tx_raw` DB
(deserializes the previous transaction to get its output details).
Spent-by info comes from the `spent_by` DB.

Fields not yet implemented (low priority):
- `scriptSig.asm` / `scriptPubKey.asm` -- script disassembly
- `extraPayload` / `extraPayloadSize` -- Dash special tx payloads
- `txlock` / `chainlock` -- requires dashd RPC (works when connected)

## External Dependencies

### librustdash (`../librustdash`)

Path dependency with `x11` feature flag enabled. Provides:
- `Block`, `BlockHeader`, `Transaction`, `TxIn`, `TxOut`, `OutPoint`
- `BlockHeader::block_hash()` (X11), `Transaction::txid()` (SHA-256d)
- `Block::serialize()`, `Block::deserialize()`
- `analyze_script()`, `encode_address()`, `decode_address()`
- `sha256d()`, `x11_hash()`, `reverse_hash()`, `hash_to_display()`

Key field names: `TxIn.previous_output`, `OutPoint.hash`, `OutPoint.n`,
`TxOut.script_pubkey`, `TxOut.value`, `Transaction.version` (i16),
`Transaction.tx_type` (DashTxType enum), `Transaction.lock_time`,
`Transaction.inputs`, `Transaction.outputs`,
`Transaction.extra_payload`.

### x11-hash (`~/projects/x11-hash`)

C library for X11 hashing. `libx11.a` must be pre-built with
`make libx11.a`. Used by librustdash via FFI behind the `x11` feature.

### dash.conf (dev machine)

Mainnet, `txindex=1`, `addressindex=1`, `rpcuser=dashrpc`. DashCore
datadir at `~/Library/Application Support/DashCore/`.

## Performance Notes

- **LMDB batched writes:** 95x speedup (187 blk/s to 17,797 blk/s)
- **Slim UTXO + compaction:** 48.2% DB size reduction (806 MB to 417 MB)
- **Read-ahead pipeline:** ~14% throughput improvement (3,288 to
  3,734 blk/s), CPU utilization 62% to 72%
- **Batch size default:** 1000 blocks per LMDB transaction
- **Read-ahead buffer:** 500 parsed blocks in bounded channel

## Backlog

See `docs/IDEAS.md` for lower-priority items. Key next steps:

- Pagination for address tx/utxo endpoints
- Balance calculation endpoint (`/api/addr/{addr}/balance`)
- `scriptSig.asm` / `scriptPubKey.asm` -- script disassembly
- `extraPayload` / `extraPayloadSize` -- Dash special tx payloads
- WebSocket / real-time event notifications
- Auto-reload DB on file change in serve mode
- `isMainChain` field (meaningful with reorg support)

## Related Projects

- **librustdash** -- `/Users/nathan/projects/dash/librustdash`
  (branch `reorgs`). Report: `librustdash/docs/REPORT_LIBRUSTDASH.md`
- **dmtv3** -- `/Users/nathan/projects/dmtv3` (Dash Masternode Tool).
  Report: `librustdash/docs/REPORT_DMTV3.md`
- **Dash Core** -- `/Users/nathan/projects/dash/librustdash` is inside
  the dashpay/dash fork repo. Parent CLAUDE.md at
  `/Users/nathan/projects/dash/CLAUDE.md` covers the C++ codebase.
