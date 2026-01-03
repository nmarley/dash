# daino

A Dash blockchain block and undo file reader.

## Overview

daino is a minimal tool to read and parse Dash block files (blk*.dat) and undo files (rev*.dat) using the librustdash primitives library. This is a proof-of-concept implementation - no indexing, no database, just raw file parsing.

## Features

- Read blocks from Dash Core's blk*.dat files
- Read undo/reorg data from Dash Core's rev*.dat files
- Support for mainnet, testnet, and regtest networks
- Parse and display block header information
- Show coinbase transaction details
- Extract and display coinbase messages (e.g., the genesis block message)
- Display spent outputs (UTXOs) from undo data for blockchain reorganization
- Read multiple blocks/undo records sequentially

## Usage

### Reading Block Files

```bash
# Read the first block from a block file
daino /path/to/.dashcore/blocks/blk00000.dat

# Read first 10 blocks
daino -c 10 /path/to/.dashcore/blocks/blk00000.dat

# Read from testnet
daino -n testnet /path/to/.dashcore/testnet3/blocks/blk00000.dat

# Read from regtest
daino -n regtest /path/to/.dashcore/regtest/blocks/blk00000.dat

# Show coinbase message (useful for viewing the genesis block message)
daino --show-coinbase-message /path/to/.dashcore/blocks/blk00000.dat
```

### Reading Undo Files

```bash
# Read undo data from rev*.dat files
daino --undo /path/to/.dashcore/blocks/rev00000.dat

# Read first 5 undo blocks
daino --undo -c 5 /path/to/.dashcore/blocks/rev00001.dat

# Read undo data from testnet
daino --undo -n testnet /path/to/.dashcore/testnet3/blocks/rev00000.dat
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
    Type: Normal
    Inputs: 1
    Outputs: 1
```

With `--show-coinbase-message` on the genesis block:

```
=== Block 0 ===
  ...
  Coinbase message:
    "Wired 09/Jan/2014 The Grand Experiment Goes Live: Overstock.com Is Now Accepting Bitcoins"
```

### Undo File Output

```
Reading undo data from: "data/rev00001.dat"
Network: Mainnet
Magic bytes: 0xBF0C6BBD

=== Undo Block 0 ===
  Position in file: 0 bytes
  Transaction undo count: 2
  Transaction 1 (non-coinbase):
    Spent outputs count: 3
      Output 0:
        Value: 5 satoshis
        ScriptPubKey: 25 bytes
        ScriptPubKey (hex): 76a914d7400c0387e6e6d5d5878db22487466dfe2d4f0888ac
        Height: 69705
        Is coinbase: false
      ...
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

## File Formats

### Block Files (blk*.dat)

Block files contain sequential blocks with the format:
- Magic bytes (4 bytes, big-endian): `0xBF0C6BBD` (mainnet)
- Size (4 bytes, little-endian): block size
- Block data: serialized block including header and all transactions

### Undo Files (rev*.dat)

Undo files contain data needed to reverse blocks during chain reorganizations:
- Magic bytes (4 bytes, big-endian): `0xBF0C6BBD` (mainnet)
- Size (4 bytes, little-endian): undo data size
- Undo data: CBlockUndo containing spent outputs (Coins) for each non-coinbase transaction
- Checksum (32 bytes): SHA256(prevBlockHash + undoData)

**Note:** The previous block hash is NOT stored in the undo file - it must be provided separately for checksum verification. Currently, daino skips checksum verification when reading undo files standalone.

Each undo block contains:
- Vector of CTxUndo (one per non-coinbase transaction)
  - Each CTxUndo contains a vector of Coins (spent outputs)
    - Each Coin has: value, scriptPubKey, block height, isCoinbase flag

## Dependencies

- `librustdash` - Dash primitives library (block/transaction parsing)
- `clap` - CLI argument parsing
- `byteorder` - Little-endian integer reading
- `hex` - Hex encoding for display
- `anyhow` - Error handling
- `sha2` - SHA256 hashing for undo file checksums

## License

MIT
