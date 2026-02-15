# Vision: Rust Validator and the Zaino Architecture

## Context

Daino is modeled after Zaino, the Zcash blockchain indexer. Zaino sits
alongside Zebra (a Rust reimplementation of the Zcash consensus
validator) and acts as a read-only indexer and API layer. Zebra owns the
canonical chain state; Zaino doesn't need to store raw blocks or
transactions because it can always ask Zebra for them via
`ReadStateService` (direct process-level access) or JSON-RPC.

Today, Dash has no Zebra equivalent -- Dash Core (C++, forked from
Bitcoin Core) is the only consensus validator. This means Daino must be
self-contained: it stores its own indexes, its own UTXO set, and (as of
the transaction detail work) its own copy of raw transaction data.

This document lays out the path from where we are today to a
Zaino-style architecture where Daino becomes a thin indexer over a Rust
validator.

## Why Dash can do this

The "never touch consensus code" orthodoxy comes from Bitcoin's specific
situation: a $1T+ network with no central coordination and an ideology
of protocol ossification. Dash is in a fundamentally different position:

- **Small, tight-knit developer community** that can coordinate quickly
- **Dash Core Group** is a centralized-enough development org to push a
  fix fast if a consensus divergence appeared
- **Masternode governance layer** provides coordination infrastructure
  Bitcoin doesn't have
- **Regular protocol upgrades already happen** -- Dash is not pretending
  the protocol is frozen in amber
- **Smaller market cap and attack surface** -- the risk/reward calculus
  for a rewrite is different than Bitcoin's

Zcash made this bet with Zebra and it paid off. Dash has every
structural advantage Zcash had, and some additional ones (simpler
transaction model -- no shielded transactions, no Sapling/Orchard
circuits, no note commitment trees).

## Current state: building blocks

Daino and librustdash are already building out pieces that a Rust
validator would eventually need:

| Component                     | Status         | Location       |
|-------------------------------|----------------|----------------|
| Block/tx deserialization      | Complete       | librustdash    |
| X11 block hashing             | Complete       | x11-hash + librustdash |
| SHA-256d (txid)               | Complete       | librustdash    |
| Script analysis               | Basic          | librustdash    |
| Special tx payloads           | Partial        | librustdash    |
| UTXO set management           | Complete       | daino-state    |
| Block file reading            | Complete       | daino-core     |
| Undo data reading             | Complete       | daino-core     |
| Chain ordering/fork resolution| Complete       | dainod         |
| Difficulty/chainwork          | Complete       | daino-core     |

Every piece of consensus logic added to librustdash is dual-use: it
serves Daino today and a future validator tomorrow.

## What a Rust validator would need

A minimal viable Rust consensus validator for Dash requires:

1. **Full script verification** -- either bind to `libbitcoinconsensus`
   via FFI or reimplement. This is the hardest single piece. Bitcoin's
   `rust-bitcoinconsensus` crate exists and could be a starting point,
   but Dash scripts have some differences.

2. **P2P networking** -- peer discovery, block/transaction relay,
   version handshaking. Could build on `tokio` and model after Zebra's
   network layer.

3. **Mempool management** -- transaction validation, fee estimation,
   eviction policies, replacement logic.

4. **Consensus rule enforcement** -- subsidy schedule, block size/weight
   limits, timestamp rules, BIP activation logic, Dash-specific
   activation (DIP enforcement heights).

5. **Deterministic masternode lists** -- the core of Dash's identity
   layer. Requires processing ProRegTx, ProUpServTx, ProUpRegTx,
   ProUpRevTx special transactions and maintaining the masternode list
   as consensus state. Uses Immer (immutable data structures) in C++;
   Rust has equivalent crates (`im`, `rpds`).

6. **LLMQ/quorum logic** -- quorum formation, distributed key
   generation session tracking, ChainLock validation, InstantSend lock
   validation.

7. **State exposure** -- the equivalent of Zebra's `ReadStateService`.
   A well-defined interface that Daino (and other consumers) can use to
   query finalized chain state, non-finalized best chain, and mempool.

The masternode layer (items 5-6) adds complexity that Zcash doesn't
have, but it's well-documented deterministic logic, not cryptographic
circuits.

### Effort estimate

Zebra was roughly 3 years of work by a well-funded Zcash Foundation
team. A Dash equivalent would likely be smaller in scope (no shielded
transactions) but larger in some areas (masternode consensus). A
realistic estimate: 2-3 engineer-years for a minimal viable validator,
assuming librustdash and Daino continue to mature the foundational
components.

## The transition: three phases

### Phase 1: Self-contained Daino (current)

Daino indexes everything it needs into LMDB and serves its REST API
without runtime dependencies on blk*.dat files or dashd (except for
live data like ChainLock status and mempool, which come from dashd via
RPC).

Key design decision: raw transaction bytes are stored in a `tx_raw`
LMDB database. This is the authoritative source for building full
transaction API responses at query time.

The API layer accesses transaction data through a `TxProvider` trait
abstraction:

```rust
/// Abstraction over raw transaction retrieval.
///
/// Backed by LMDB today, by a validator state service tomorrow.
trait TxProvider {
    fn get_raw_tx(&self, txid: &[u8; 32]) -> Result<Option<Vec<u8>>>;
    fn get_raw_tx_batch(&self, txids: &[[u8; 32]]) -> Result<Vec<Option<Vec<u8>>>>;
}
```

This abstraction exists specifically so Phase 3 is a backend swap, not
a rewrite.

### Phase 2: librustdash as consensus foundation

Continue building out librustdash with consensus-relevant logic:

- Full special transaction payload support (all ProTx variants, LLMQ
  commitment, governance payloads)
- Script verification (start with FFI to libbitcoinconsensus, consider
  native Rust later)
- Deterministic masternode list computation
- Block validation rules (beyond just deserialization)

Each of these is independently useful for Daino (richer API responses,
validation during indexing) while simultaneously building toward a
validator.

### Phase 3: Zaino-style architecture

When a Rust validator exists:

- Daino drops `tx_raw` storage and queries the validator's state
  service for raw transaction data on demand
- The `TxProvider` trait gets a new implementation backed by the
  validator
- Daino's role simplifies to: maintain derived indexes (address
  history, UTXO set, spent-by tracking, search indexes) and serve the
  REST API
- The validator owns canonical chain state, UTXO set, mempool
- Block file reading moves entirely to the validator

The LMDB index shrinks significantly (no more raw tx storage) and Daino
becomes a lightweight, fast-to-rebuild derived view over the validator's
state -- exactly what Zaino is to Zebra.

## Design principles for today

To make the Phase 1 -> Phase 3 transition smooth, follow these
principles now:

1. **Trait abstractions at data boundaries.** Any code that retrieves
   raw chain data (blocks, transactions) should go through a trait, not
   directly into LMDB. The current implementation backs the trait with
   LMDB; a future one backs it with a state service.

2. **librustdash is the shared foundation.** Consensus types, hashing,
   serialization, and (eventually) validation logic belong in
   librustdash, not in Daino. Both the indexer and a future validator
   import the same library.

3. **Indexes over embedded data.** AGENT.md already says this. Daino's
   LMDB databases should be derived indexes that can be rebuilt from
   the authoritative source (today: raw tx bytes; tomorrow: validator
   state). Don't cache derived data that's cheap to recompute.

4. **Don't design around blk*.dat.** The `index` command reads blk*.dat
   as a bulk import optimization, but the architecture shouldn't assume
   blk*.dat availability at serve time. The `follow` command (RPC-based)
   is architecturally closer to the future state.

## Comparison: Zcash stack vs. future Dash stack

| Layer           | Zcash               | Dash (today)        | Dash (future)       |
|-----------------|----------------------|---------------------|---------------------|
| Validator       | Zebra (Rust)        | Dash Core (C++)     | Rust validator      |
| Indexer         | Zaino (Rust)        | Daino (Rust)        | Daino (Rust)        |
| Primitives lib  | librustzcash        | librustdash         | librustdash         |
| State interface | ReadStateService    | blk*.dat + RPC      | ReadStateService    |
| Index storage   | (minimal/cache)     | LMDB (full)         | LMDB (derived only) |

## References

- [Zaino repository](https://github.com/zingolabs/zaino)
- [Zebra repository](https://github.com/ZcashFoundation/zebra)
- [Blockstream electrs](https://github.com/Blockstream/electrs) --
  stores raw txs in RocksDB (`T{txid}` -> serialized tx), the approach
  Daino follows for Phase 1
- [Esplora API](https://github.com/Blockstream/esplora/blob/master/API.md) --
  best-in-class REST API design (amounts as integer satoshis, spent
  tracking as separate endpoints, clean separation of concerns)
