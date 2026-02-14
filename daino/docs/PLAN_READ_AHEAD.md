# Plan: Read-Ahead Pipeline

## Problem

The current indexer is single-threaded: read block from disk, parse it,
compute hashes, serialize to LMDB, repeat. Disk I/O and CPU work are
serialized, leaving both underutilized.

## Solution: Pipelined Reader + Writer

Separate block reading/parsing from LMDB writing using a bounded channel:

```
Reader thread(s)          Channel           Writer thread
------------------    -->  [...]  -->    ------------------
Read blk file                            Receive parsed block
Deserialize block                        Build BlockBatch
Compute block_hash                       Write to LMDB
Compute txids
Send to channel
```

### Reader Thread

- Reads blocks in chain order (using the height map from the chain
  ordering pass 1)
- Deserializes, computes X11 block hash and SHA-256d txids
- Sends `(height, ParsedBlock)` tuples through a bounded channel
- Bounded channel provides backpressure (reader doesn't get too far
  ahead of writer)

### Writer Thread (main thread)

- Receives parsed blocks from the channel
- Builds BlockRecord, TxRecord, UtxoEntry, etc.
- Batches into BlockBatch, flushes to LMDB every N blocks
- UTXO tracking must remain single-threaded (sequential chain order)

### Why Not Multiple Reader Threads?

Multiple reader threads reading different blk files simultaneously is
possible but adds complexity:

- Need a reordering buffer (blocks arrive out of height order)
- File I/O may not benefit from parallelism on spinning disks (seeks
  kill throughput)
- On SSDs, a single reader thread is likely fast enough to keep the
  writer saturated

Start with one reader thread. If profiling shows the reader is the
bottleneck (unlikely -- X11 hashing is cheap via FFI to C), we can add
more.

## Bounded Channel Sizing

A reasonable buffer: 500-1000 blocks. Each parsed block is maybe 1-10 KB
in memory (header + transaction metadata), so 500 blocks ~= 5 MB. This
keeps the reader ~500 blocks ahead of the writer without using
significant RAM.

## Dependencies

- Requires chain ordering (PLAN_CHAIN_ORDERING.md) to be implemented
  first, since the reader needs to know which block to read next in
  chain order.

## Changes Required

### dainod (`dainod/src/commands/`)

1. **`index.rs`**: Refactor pass 2 to spawn a reader thread that sends
   parsed blocks through a `crossbeam-channel` or `std::sync::mpsc`.
   Main thread becomes the writer.

### Cargo.toml

2. May need `crossbeam-channel` dependency (or use std mpsc).

### daino-core

3. No changes needed if the reader thread uses the existing
   `BlockFileReader::read_block_at()` from the chain ordering work.

## Not In Scope

- Multiple reader threads (defer until profiling shows need)
- Parallel LMDB writes (LMDB is single-writer by design)
- Async I/O (block file reads are better served by threads than async)
