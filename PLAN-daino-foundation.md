# PLAN: Daino Foundation Hardening

## Goal

Make daino a trustworthy, reversible chain index on top of librustdash
before adding more API surface. Foundation first: one index pipeline,
connect/disconnect semantics, clean data boundaries, and a correctness
gate against dashd. Feature work (pagination, balance, ZMQ polish)
comes after.

## Context

Branch tip: `r2` (Mar 2026). librustdash Phase 1 primitives are solid.
daino already has a two-pass chain-ordered indexer, LMDB indexes
(including `tx_raw` and `spent_by`), Insight-aligned block/tx APIs, and
undo file reading during forward index. What is missing is reversible
state, a single apply path, and the `TxProvider` seam described in
`daino/docs/VISION_RUST_VALIDATOR.md`.

## Non-goals

- Rust consensus validator (Zebra equivalent)
- Pagination / balance endpoints (`PLAN_PAGINATION_BALANCE.md` deferred)
- Full ZMQ production wiring (poll follow is enough until reorg works)
- Masternode / governance / InstantSend specialized indexes
- Rewriting librustdash into a multi-crate workspace
- Merging `r2` into upstream `develop`

## Principles

1. Indexes are derived and must be fully reversible from connect/disconnect.
2. One code path applies a block whether it came from `blk*.dat` or RPC.
3. Raw chain data access goes through traits, not direct LMDB calls.
4. Schema changes require wipe + re-index (already policy in AGENTS.md).
5. Tests stay fast: synthetic data via `put_batch` / helpers; no new
   tests that scan large real `blk*.dat` files.
6. librustdash stays the shared primitives home; daino does not grow
   consensus types.

## Stage 1: Re-verify and document truth

1a: Run `cargo test --workspace` in `librustdash` and `daino`; fix any
    bitrot from the months away (deps, paths, x11-hash build).
1b: Confirm `~/projects/x11-hash` builds (`make libx11.a`) and the path
    dep in `librustdash/Cargo.toml` still resolves.
1c: Rewrite `daino/README.md` so it describes the real indexer + API
    (stop claiming "no indexing").
1d: Refresh `librustdash/docs/REPORT_DAINO.md` and
    `librustdash/docs/REPORT_LIBRUSTDASH.md` to match `r2` reality
    (tx parity done, undo in indexer, remaining gaps listed).
1e: Add a short "Current gaps" section to `daino/AGENTS.md` pointing at
    this plan (reorg, TxProvider, dual index path) so agents do not
    treat pagination as next work.

## Stage 2: Single block-apply pipeline

2a: Extract a shared module (e.g. `daino-state` or a small
    `daino-index` helper used by both CLI commands) that takes a
    deserialized `Block` plus height/hash/size/chainwork/optional undo
    and produces a `BlockBatch` (all index writes: block, txs, tx_raw,
    utxos, addr indexes, spent_by).
2b: Move address/script extraction and spent-by construction out of
    `dainod/src/commands/index.rs` into that shared path.
2c: Refactor `dainod/src/commands/index.rs` Pass 2 writer to call the
    shared apply path only.
2d: Refactor `dainod/src/commands/follow.rs` `index_fetched_block` to
    call the same path (including tx_raw + spent_by; today follow is
    thinner).
2e: Unit tests on the shared apply path with synthetic blocks covering
    coinbase-only and a simple spend.

## Stage 3: Disconnect / reorg foundation

3a: Define the reverse of every `put_batch` write: remove block record,
    hash_to_height, block_txs, txs_by_id, tx_raw, spent_by entries,
    addr_to_txs entries; restore UTXOs spent by the block (from undo or
    spent_by + tx_raw); remove UTXOs created by the block; fix
    addr_utxos; update ChainMeta tip/counts.
3b: Implement `DainoDB::disconnect_tip` (or `disconnect_block`) as one
    LMDB transaction, mirrored with `put_batch` batching style.
3c: Implement `DainoDB::disconnect_to_height(height)` that walks tip
    down to a fork point.
3d: Wire follower reorg detection: when a new block's prev hash is not
    the indexed tip hash, walk back to common ancestor (via RPC headers
    and local hash_to_height), disconnect orphaned blocks, then apply
    the new branch through the shared apply path.
3e: Tests: connect N synthetic blocks, disconnect one, assert UTXO set
    and addr indexes match pre-connect state; multi-block disconnect;
    simple fork (A-B vs A-C) via apply/disconnect helpers without
    real dashd.

## Stage 4: TxProvider trait boundary

4a: Introduce `TxProvider` (as sketched in
    `daino/docs/VISION_RUST_VALIDATOR.md`) in daino-core or daino-state:
    `get_raw_tx`, optional `get_raw_tx_batch`.
4b: Implement `LmdbTxProvider` backed by the existing `tx_raw` DB.
4c: Change `daino-serve` tx/vin resolution to use `TxProvider` only
    (no direct `db.get_raw_tx` in handlers).
4d: Keep the trait small; do not invent a full ReadStateService yet.
4e: Tests: serve-layer or provider unit tests with synthetic raw txs.

## Stage 5: Correctness gate

5a: Document a manual (or scripted) verification procedure:
    index a known mainnet range from `blk*.dat`, compare sample
    block/tx/utxo/addr responses to dashd RPC (and Insight if available).
5b: Add a `dainod` subcommand or dev tool (e.g. `verify`) that checks
    random or listed heights: local block hash, tx count, and a few
    txids against dashd RPC when `--rpc-*` is provided.
5c: Run a longer local index (as far as machine/time allow), record
    tip height, DB size, blk/s in AGENTS.md performance notes if numbers
    change materially.
5d: Fix any correctness bugs found before calling foundation done.

## Stage 6: Hygiene and stop line

6a: Ensure follow and index produce identical DB contents for the same
    chain segment (spot-check or small automated compare on regtest
    fixtures / synthetic data).
6b: Remove or clearly mark dead code paths left after the unify
    (duplicate indexing logic, obsolete comments).
6c: Final doc pass: AGENTS.md schema/API sections still accurate;
    IDEAS.md and PLAN_PAGINATION_BALANCE.md marked as post-foundation.
6d: Stop. Foundation complete when: single apply path, disconnect/reorg
    works with tests, TxProvider in serve path, and verify gate has
    been run successfully once.

## Deferred (after this plan)

- `daino/docs/PLAN_PAGINATION_BALANCE.md`
- ZMQ real subscription (replace poll)
- WebSocket / real-time events
- `scriptSig.asm` / `scriptPubKey.asm`
- Special tx `extraPayload` in API responses
- Auto-reload LMDB in serve mode
- addr totalReceived/totalSent index
- Dash-specific indexes (masternode payments, governance, IS, ChainLock
  in-index rather than RPC passthrough)

## Success criteria

- `cargo test --workspace` green for librustdash and daino
- One shared apply path for file index and RPC follow
- Disconnect restores prior UTXO and address index state (tested)
- Follower can survive a simple reorg without wipe/reindex
- Serve layer reads raw txs only through `TxProvider`
- At least one successful cross-check of indexed data against dashd
- Docs match code; no status report claims that contradict `r2`

## References

- `daino/AGENTS.md`
- `daino/docs/VISION_RUST_VALIDATOR.md`
- `daino/docs/PLAN_CHAIN_ORDERING.md` (implemented)
- `daino/docs/PLAN_READ_AHEAD.md` (implemented)
- `daino/docs/PLAN_PAGINATION_BALANCE.md` (deferred)
- `librustdash/docs/ROADMAP.md`
- `librustdash/docs/REPORT_DAINO.md`
- `INDEXER_DESIGN.md` (historical; daino is the live design)
