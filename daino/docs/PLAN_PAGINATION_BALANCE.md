# Plan: Pagination and Balance Endpoints

## Background

The address endpoints currently return all results with no pagination.
For addresses with large transaction histories (exchange hot wallets,
mining pool payouts), this produces huge responses and high memory
usage. Insight provides pagination via `from`/`to` index parameters.

Insight also returns balance and total received/sent figures on the
address summary endpoint. These are computed by Dash Core's
`addressindex` (a LevelDB index mapping address to all value deltas).

## Insight API Reference

### Pagination

Insight uses index-based pagination with `from`/`to` query params:

```
GET /addrs/:addrs/txs?from=0&to=10
```

Response:
```json
{
  "totalItems": 523,
  "from": 0,
  "to": 10,
  "items": [ ...full tx objects... ]
}
```

Default: `from=0`, `to=from+10` for txs, `to=from+1000` for UTXOs.
UTXO pagination has a hard cap of 1000 per request.

Optional height range filters: `fromHeight`, `toHeight`.

The single-address endpoints (`/addr/:addr/utxo`) are unpaginated in
Insight. Pagination only exists on the multi-address variants. We will
add pagination to the single-address variants too, since that is where
clients actually hit us.

### Address Summary

Insight's `GET /addr/:addr` returns:

```json
{
  "addrStr": "XsomeAddr",
  "balance": 1.5,
  "balanceSat": 150000000,
  "totalReceived": 10.0,
  "totalReceivedSat": 1000000000,
  "totalSent": 8.5,
  "totalSentSat": 850000000,
  "unconfirmedBalance": 0,
  "unconfirmedBalanceSat": 0,
  "txAppearances": 42,
  "transactions": ["txid1", "txid2", ...]
}
```

Insight delegates to `node.getAddressSummary()` which calls Dash
Core's `getaddressbalance` RPC. Dash Core's `addressindex` stores
per-address value deltas and sums them at query time.

Sub-routes: `/addr/:addr/balance`, `/addr/:addr/totalReceived`,
`/addr/:addr/totalSent`, `/addr/:addr/unconfirmedBalance`.

## Design

### Stage 1: Pagination for address txs

Add `from`/`to` query params to `GET /api/addr/:addr/txs`.

DB layer: add `get_addr_txs_range()` that accepts offset and limit,
returns `(items, total_count)`. Implemented by prefix-scanning
`addr_to_txs`, counting total entries, then collecting the requested
window. LMDB prefix scans are already height-ordered (natural key
order), so the offset/limit maps directly to skip/take on the
iterator.

API layer: parse `from`/`to` from query string (default: 0, 10).
Return:

```json
{
  "totalItems": 523,
  "from": 0,
  "to": 10,
  "items": ["txid1", "txid2", ...]
}
```

When no `from`/`to` given, return the first 10 items (not all items).
This is a breaking change from the current behavior but matches
Insight and prevents accidental full scans.

### Stage 2: Pagination for address UTXOs

Same pattern for `GET /api/addr/:addr/utxo`.

DB layer: add `get_addr_utxos_range()` with offset/limit, returns
`(items, total_count)`.

Default: `from=0`, `to=from+1000`. Hard cap: 1000 per request.

Response:

```json
{
  "totalItems": 2500,
  "from": 0,
  "to": 1000,
  "items": [{ "txid": "...", "vout": 0, "satoshis": 100000000, ... }]
}
```

### Stage 3: Balance endpoint

Add `GET /api/addr/:addr/balance`.

Compute by summing all UTXOs for the address (we have the
`addr_utxos` + `utxos` databases). This gives `balanceSat`. Derive
`balance` as `balanceSat / 1e8`.

DB layer: add `get_addr_balance()` that prefix-scans `addr_utxos`,
looks up each UTXO value, and sums. Returns `i64` (satoshis).

Response:

```json
{
  "addrStr": "XsomeAddr",
  "balanceSat": 150000000,
  "balance": 1.5
}
```

### Stage 4: Enrich address summary

Update `GET /api/addr/:addr` to include balance fields:

```json
{
  "addrStr": "XsomeAddr",
  "balance": 1.5,
  "balanceSat": 150000000,
  "txAppearances": 42,
  "transactions": ["txid1", ...]
}
```

The `transactions` list should respect the same default pagination
(first 10 txids). Include `totalReceived`/`totalSent` only if we
decide to add a dedicated index (see below).

### Deferred: totalReceived / totalSent

Computing `totalReceived` and `totalSent` requires walking every
transaction for the address and summing input/output values. Two
options:

1. **Query-time computation:** Walk `addr_to_txs`, look up each raw
   tx, sum outputs to this address (received) and inputs from this
   address (sent). Expensive for active addresses.

2. **Index-time computation:** Store a running `addr_balance` record
   per address during indexing: `{ totalReceived, totalSent }`. Adds
   one DB write per address per block. Very fast at query time.

Option 2 is better for production. Requires a new LMDB database
(`addr_balance`) and changes to `put_batch()`. This is a schema
change requiring re-index. Defer to a future stage.

### Tests

Each stage should include:

- Unit test in `daino-state/src/db.rs` for the new DB method
  (synthetic data via `put_batch()`)
- Integration test in `dainod/tests/api_integration.rs` for the API
  endpoint (indexes real blocks, queries HTTP, asserts response shape)

No new tests that open real `blk*.dat` files. Use existing indexed
test data.
