# daino

A simple Dash blockchain block file reader.

## Overview

daino is a minimal tool to read and parse Dash block files (blk*.dat) using the librustdash primitives library. This is a proof-of-concept implementation - no indexing, no database, just raw block file parsing.

## Features

- Read blocks from Dash Core's blk*.dat files
- Support for mainnet, testnet, and regtest networks
- Parse and display block header information
- Show coinbase transaction details
- Read multiple blocks sequentially

## Usage

```bash
# Read the first block from a block file
daino /path/to/.dashcore/blocks/blk00000.dat

# Read first 10 blocks
daino -c 10 /path/to/.dashcore/blocks/blk00000.dat

# Read from testnet
daino -n testnet /path/to/.dashcore/testnet3/blocks/blk00000.dat

# Read from regtest
daino -n regtest /path/to/.dashcore/regtest/blocks/blk00000.dat
```

## Example Output

```
Reading blocks from: "/path/to/blk00000.dat"
Network: Mainnet
Magic bytes: 0xBF0C6BBD

=== Block 0 ===
  Position in file: 293 bytes
  Version: 1
  Previous block: 0000000000000000000000000000000000000000000000000000000000000000
  Merkle root: e0028eb9648db56b1ac77cf090b99048a8007e2bb64b68f092c03c7f56a662c7
  Timestamp: 1390095618
  Bits: 0x1E0FFFF0
  Nonce: 28917698
  Transaction count: 1
  Coinbase tx:
    Version: 1
    Type: Classical
    Inputs: 1
    Outputs: 1
```

## Building

```bash
cargo build --release
```

The binary will be at `target/release/daino`.

## Architecture

This is a minimal implementation following the architecture outlined in `docs/DAINO_ARCHITECTURE.md` (in the librustdash repository). The full daino indexer will include:

- LMDB/RocksDB storage
- Address indexing
- Masternode tracking
- ChainLock tracking
- gRPC API server

For now, this is just a simple block file reader to validate that librustdash can correctly parse Dash block data.

## Dependencies

- `librustdash` - Dash primitives library (block/transaction parsing)
- `clap` - CLI argument parsing
- `byteorder` - Little-endian integer reading
- `hex` - Hex encoding for display
- `anyhow` - Error handling

## License

MIT
