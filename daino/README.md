# daino

A modern Rust blockchain indexer for Dash. Modeled after Zaino (the Zcash
indexer): a read-only companion that maintains its own LMDB index and serves
an Insight-compatible REST API. Built on [librustdash](../librustdash)
primitives.

## Status

Working MVP on branch `r2`. Block and transaction API responses match
Insight for core fields. Foundation hardening (single apply path, reorg
disconnect, `TxProvider`) is tracked in `../PLAN-daino-foundation.md`.

## Features

- Two-pass chain-ordered indexing from Dash Core `blk*.dat` files
- Undo (`rev*.dat`) reading during index for input-side address data
- LMDB storage: blocks, txs, raw tx bytes, UTXOs, address history, spent-by
- Insight-compatible REST API (blocks, txs, addresses)
- Optional dashd JSON-RPC for tip follow and live endpoints (ChainLock,
  sporks, governance)
- Read-ahead pipeline and batched LMDB writes

## Workspace

| Crate | Purpose |
|-------|---------|
| `daino-core` | Block/undo file reading, difficulty and chainwork |
| `daino-state` | LMDB-backed indexes |
| `daino-fetch` | dashd JSON-RPC client, ZMQ scaffold, tip follower |
| `daino-serve` | Axum REST API |
| `dainod` | CLI binary |

## Build

Requires the x11-hash C library at `~/projects/x11-hash` (path
dependency from librustdash). Build it first:

```bash
make -C ~/projects/x11-hash libx11.a
cargo build --workspace
cargo test --workspace
```

## CLI

```bash
# Index blocks from a directory of blk*.dat files
cargo run -p dainod -- index \
  --datadir /path/to/blocks \
  --dbdir /tmp/daino-db

# Show database status
cargo run -p dainod -- status --dbdir /tmp/daino-db

# Serve REST API (default port 3141)
cargo run -p dainod -- serve --dbdir /tmp/daino-db --port 3141

# Follow a running dashd tip via RPC poll
cargo run -p dainod -- follow \
  --dbdir /tmp/daino-db \
  --rpc-url http://127.0.0.1:9998 \
  --rpc-user user \
  --rpc-password pass

# Compact LMDB (reclaim dead pages)
cargo run -p dainod -- compact --dbdir /tmp/daino-db

# Cross-check index against dashd RPC
cargo run -p dainod -- verify --dbdir /tmp/daino-db \
  --rpc-url http://127.0.0.1:9998 --rpc-user dashrpc --rpc-password secret

# Read raw blk/rev files (debug)
cargo run -p dainod -- read /path/to/blk00000.dat
cargo run -p dainod -- read --undo /path/to/rev00000.dat
```

See `docs/VERIFY.md` for the correctness gate procedure.

`--datadir` is the directory that directly contains `blk*.dat` (no
extra `blocks/` suffix assumed).

## REST API (DB-backed)

| Endpoint | Description |
|----------|-------------|
| `GET /api/status` | Chain tip, block/tx counts |
| `GET /api/block/:hash` | Full block (Insight-shaped) |
| `GET /api/block-index/:height` | Height to hash |
| `GET /api/tx/:txid` | Full tx with vin/vout, spent-by |
| `GET /api/addr/:addr` | Address summary |
| `GET /api/addr/:addr/txs` | Address tx history |
| `GET /api/addr/:addr/utxo` | Address UTXOs |

With `--rpc-*` on `serve`: `/api/chainlock`, `/api/sporks`,
`/api/governance/list`.

## Architecture notes

- Block files store blocks in received order, not chain order. Pass 1
  scans headers; a chain walk assigns heights; Pass 2 reads full blocks
  in chain order.
- Dash uses X11 for block hashes and SHA-256d for txids (via librustdash).
- Schema changes require deleting the DB and re-indexing.
- Agent/developer context: see `AGENTS.md`.
- Long-term path (Rust validator + thin indexer):
  `docs/VISION_RUST_VALIDATOR.md`.

## Known gaps

- ZMQ subscriber is scaffolded; follow mode polls RPC
- Pagination and balance endpoints planned but deferred
  (`docs/PLAN_PAGINATION_BALANCE.md`)

Foundation items done: shared apply path, disconnect/reorg, TxProvider,
`dainod verify`. See `../PLAN-daino-foundation.md`.

## License

MIT
