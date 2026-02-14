# Plan: Chain Ordering Correctness

## Problem

The current indexer (`dainod index`) assigns block heights sequentially as
it reads blocks from `blk*.dat` files. This is wrong. Block files contain
blocks in *received* order, not chain order. The first file starting from
genesis happens to be mostly in order, but past the first file or two,
heights will be incorrect. Wrong heights mean wrong UTXO state, wrong
address histories, wrong everything.

## Solution: Two-Pass Indexer

### Pass 1: Header Scan

Read all `blk*.dat` files sequentially. For each block, read only the
80-byte header (skip the transaction data). Collect:

```
HashMap<block_hash, HeaderEntry>

struct HeaderEntry {
    prev_hash: [u8; 32],
    file_num: u32,
    file_offset: u64,   // byte offset of the block's magic bytes
    block_size: u32,     // full block size (for seeking in pass 2)
}
```

This is fast because we read 80 bytes and skip `block_size - 80` bytes
for each block, rather than deserializing all transactions.

### Chain Walk

Starting from genesis (the block whose `prev_hash` is all zeros), follow
the linked list forward:

```
genesis -> block1 -> block2 -> ... -> tip
```

This produces a `Vec<[u8; 32]>` of block hashes in chain order, where the
index IS the height. Also produces a `HashMap<block_hash, u32>` for
height lookups.

If there are forks, the longest chain wins (most blocks). Orphan blocks
(blocks whose parent we never saw) are skipped.

### Pass 2: Full Index

Re-read blocks in chain order. For each height 0..tip:
1. Look up which file and offset contains that block
2. Seek to that offset, read and deserialize the full block
3. Index it (same as current logic: BlockRecord, TxRecord, UTXOs, etc.)

This requires `BlockFileReader` to support seeking to a specific offset.
We add a method like `read_block_at(offset) -> Block` or extend the
reader to accept a seek position.

Batching works the same as before: collect N blocks into a `BlockBatch`,
flush to LMDB.

## Changes Required

### daino-core (`crates/daino-core/`)

1. **`block_reader.rs`**: Add `read_header_at(offset)` that reads just
   the 80-byte header (skipping magic + size prefix). Add
   `read_block_at(offset)` that seeks to an offset and reads a full
   block. Both need the reader to use `Seek`.

2. **New `header_scan.rs`** (or inline in block_reader): A function that
   scans an entire blk file and returns a `Vec<ScannedHeader>` with
   block_hash, prev_hash, file_offset, block_size for each block.

### daino-state (`crates/daino-state/`)

No changes needed. The DB layer already accepts blocks with explicit
heights via `BlockBatch`.

### dainod (`dainod/src/commands/`)

3. **`index.rs`**: Rewrite to two-pass architecture:
   - Pass 1: scan all files, build header map
   - Chain walk: derive height assignments
   - Pass 2: read full blocks in chain order, index with correct heights

## Resumption

For resumption, if `tip_height` is already N, we still need pass 1 (to
build the full chain map), but in pass 2 we skip blocks 0..N and start
indexing from N+1. Pass 1 is fast (header-only) so repeating it on
resume is acceptable.

## Not In Scope

- Parallel file reads (see PLAN_READ_AHEAD.md)
- Reorg handling (for now, we only index once; `follow` mode handles
  reorgs separately via dashd RPC)
- Orphan block storage (orphans are simply skipped)
