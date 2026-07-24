# Verifying the Daino index against dashd

This is the correctness gate for the foundation plan: prove that
indexed block hashes, transaction counts, and txids match a trusted
dashd RPC for a sample of heights.

## Prerequisites

1. A populated Daino LMDB (`dainod index ...`).
2. A synced dashd with RPC enabled (`txindex` helpful but not required
   for block-level checks).
3. RPC credentials matching dashd's `rpcuser` / `rpcpassword`.

## Automated check

```bash
# Index a range first (example: first blocks from a blocks/ directory)
cargo run -p dainod -- index \
  --datadir /path/to/DashCore/blocks \
  --dbdir /tmp/daino-db \
  --max-blocks 10000

# Cross-check against dashd
cargo run -p dainod -- verify \
  --dbdir /tmp/daino-db \
  --rpc-url http://127.0.0.1:9998 \
  --rpc-user dashrpc \
  --rpc-password 'secret' \
  --samples 32 \
  --height 1 \
  --height 1000
```

What is compared at each sampled height:

- Block hash (display-order hex, same as `getblockhash`)
- Transaction count
- Full ordered txid list (`getblock` verbosity=1 `tx` array)

Exit status is non-zero on any mismatch.

Sampling always includes height 0 and the shared tip (min of local and
dashd tips). Additional heights are spaced evenly; `--height` adds
explicit points.

## Manual spot checks (optional)

With `dainod serve` running:

```bash
# Local
curl -s http://127.0.0.1:3141/api/block-index/0
curl -s http://127.0.0.1:3141/api/status

# dashd
dash-cli getblockhash 0
dash-cli getblockcount
```

Compare a known address UTXO set or a single txid via `/api/tx/:txid`
against `getrawtransaction` if deeper confidence is needed.

## When tips differ

If the local index is behind dashd, verify only checks `0..=local_tip`.
If local is ahead (should be rare), the shared range is `0..=dashd_tip`.
Catch up with `dainod follow` or re-index before treating a full tip
match as proven.

## Recording a successful gate

After a green verify run, note in the session or AGENTS performance
section: local tip height, sample count, and that verify passed. No
need to commit RPC credentials or machine-specific paths.
