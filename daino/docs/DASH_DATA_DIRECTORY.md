# Dash Core Data Directory Reference

A quick reference guide to understanding the files and directories in a Dash Core node's data directory.

---

## Block Storage Files

### `blk*.dat` files
- **What**: Raw blockchain data files containing actual blocks
- **Format**: Sequential blocks with magic bytes + size + block data
- **Size**: Each file is ~128 MB (Dash Core splits blocks across multiple files)
- **Contents**: Complete blocks including all transactions
- **Example**: `blk00000.dat` contains the genesis block and early blocks
- **Sequential**: `blk00000.dat` → `blk00001.dat` → ... → `blk00291.dat`
- **Used by**: Block explorers, indexers, full nodes

### `rev*.dat` files
- **What**: "Undo" data for blocks (reorg/rollback support)
- **Purpose**: If a chain reorganization happens, these files help revert blocks
- **Contents**: Information needed to undo the effects of a block (spent UTXOs, etc.)
- **Pairing**: `rev00052.dat` corresponds to `blk00052.dat`
- **Why needed**: Allows the node to handle blockchain forks and reorganizations

### `index/` directory
- **What**: LevelDB database indexing the block files
- **Contents**: Maps block hashes → file position (which blk*.dat file and byte offset)
- **Purpose**: Fast block lookup without scanning all blk*.dat files
- **Structure**:
  - Block hash → (file_number, file_offset, block_height)
  - Transaction hash → (block_hash, tx_position)
- **Database**: LevelDB

---

## State & Index Directories

### `chainstate/` directory
- **What**: LevelDB database of the UTXO set (Unspent Transaction Outputs)
- **Contents**: All current unspent outputs (coins that can be spent)
- **Purpose**: Validates new transactions without scanning entire blockchain
- **Critical**: This is how nodes know what coins exist and can be spent
- **Size**: Much smaller than full blockchain (only current state, not history)
- **Database**: LevelDB
- **Performance**: Critical for transaction validation speed

### `evodb/` directory
- **What**: Dash-specific "Evolution Database"
- **Contents**: Deterministic masternode list snapshots, quorum state, governance objects
- **Purpose**: Stores Dash-specific consensus data:
  - Masternode registrations/updates (ProRegTx, ProUpServTx, etc.)
  - LLMQ quorum information
  - Platform credit pool data
  - ChainLock information
  - Asset locks/unlocks
- **Database**: LevelDB
- **Dash-Specific**: Not present in Bitcoin Core

### `indexes/` directory
- **What**: Optional transaction and address indexes
- **Contents**: (if `-txindex=1` enabled) Maps every txid → block location
- **Purpose**: Fast transaction lookup by hash without scanning blocks
- **Optional**: Only created if node runs with indexing enabled
- **Config**: Enabled with `txindex=1` in dash.conf
- **Database**: LevelDB

### `llmq/` directory
- **What**: Long-Living Masternode Quorum data
- **Contents**:
  - DKG (Distributed Key Generation) session data
  - Quorum commitments
  - Recovered signatures (InstantSend locks, ChainLocks)
  - Quorum snapshot data
- **Purpose**: Stores LLMQ-related state for Dash's advanced features
- **Dash-Specific**: Powers InstantSend, ChainLocks, governance voting
- **Database**: Mixed (LevelDB + flat files)

---

## Cache & Peer Data

### `mempool.dat`
- **What**: Serialized mempool state
- **Contents**: Unconfirmed transactions waiting to be mined
- **Purpose**: Persists mempool across node restarts
- **Updated**: On shutdown and periodically
- **Format**: Binary serialization

### `peers.dat` / `peers.7b98`
- **What**: Known peer addresses
- **Contents**: List of other Dash nodes to connect to (IP:port)
- **Purpose**: Node remembers peers across restarts
- **7b98**: Versioned peer file (different format versions)
- **Format**: Binary serialization

### `banlist.dat` / `banlist.json`
- **What**: Banned peer IP addresses
- **Contents**: IPs/subnets that misbehaved and are temporarily banned
- **Purpose**: Protect against bad actors, spam, DoS attacks
- **Format**: Binary (.dat) or JSON (.json)

### `mncache.dat`
- **What**: Masternode list cache
- **Contents**: Cached deterministic masternode list
- **Purpose**: Speeds up node startup (doesn't need to rebuild from blocks)
- **Dash-Specific**: Related to masternode system
- **Format**: Binary serialization

### `netfulfilled.dat`
- **What**: Network request tracking
- **Contents**: Tracks completed network requests to prevent spam
- **Purpose**: Rate limiting and DoS protection
- **Format**: Binary serialization

---

## Configuration & Governance

### `dash.conf`
- **What**: Node configuration file
- **Contents**: Settings like RPC credentials, network ports, enabled features
- **Format**: INI-style text file
- **Example settings**:
  ```
  rpcuser=dashrpc
  rpcpassword=<password>
  txindex=1
  listen=1
  server=1
  ```
- **Location**: Main data directory or `~/.dashcore/dash.conf`

### `governance.dat`
- **What**: Governance object cache
- **Contents**: Proposals, triggers, votes
- **Purpose**: Stores Dash governance system state
- **Dash-Specific**: Part of treasury/proposal system
- **Format**: Binary serialization

### `sporks.dat`
- **What**: Spork settings cache
- **Contents**: Network-wide feature flags controlled by core developers
- **Purpose**: Allows emergency features to be enabled/disabled network-wide
- **Examples**: SPORK_2_INSTANTSEND_ENABLED, SPORK_3_INSTANTSEND_BLOCK_FILTERING
- **Dash-Specific**: Network governance mechanism
- **Format**: Binary serialization

### `fee_estimates.dat`
- **What**: Fee estimation data
- **Contents**: Historical fee data for estimating transaction fees
- **Purpose**: Helps wallet estimate appropriate fees for timely confirmation
- **Format**: Binary serialization

---

## Logs & Runtime

### `debug.log` (and `.1`, `.2`, `.3`, `.4`, `.5`)
- **What**: Rotating log files
- **Contents**: Node operation logs, errors, warnings, info messages
- **Rotation**: When log gets too big, rotates to debug.log.1, then .2, etc.
- **Format**: Plain text
- **Useful for**: Debugging, monitoring node health
- **Control verbosity**: Use `-debug=<category>` flag

### `dashd.pid`
- **What**: Process ID file
- **Contents**: PID of running dashd process
- **Purpose**: Prevents multiple instances, allows scripts to find the process
- **Format**: Plain text (single number)
- **Deleted**: When node shuts down cleanly

### `wallet.dat`
- **What**: Wallet file (if wallet enabled)
- **Contents**: Private keys, addresses, transaction history, metadata
- **Critical**: **BACKUP THIS FILE** - contains your coins!
- **Encryption**: Can be encrypted with `encryptwallet` RPC
- **Format**: Berkeley DB
- **Note**: Disable wallet on full nodes with `-disablewallet=1`

### `settings.json`
- **What**: GUI/runtime settings
- **Contents**: User preferences that override dash.conf temporarily
- **Format**: JSON
- **Created by**: Dash-Qt GUI or runtime RPC commands

---

## Directory Structure Overview

```
.dashcore/  (or /app/data/)
├── blocks/
│   ├── blk00000.dat        # Genesis block + early blocks
│   ├── blk00001.dat        # Next ~128 MB of blocks
│   ├── ...
│   ├── blk00291.dat        # Latest blocks
│   ├── rev00000.dat        # Undo data for blk00000.dat
│   ├── ...
│   └── index/              # LevelDB: block hash → file position
│
├── chainstate/             # LevelDB: UTXO set (current state)
├── evodb/                  # LevelDB: Masternode/quorum/governance data
├── indexes/                # LevelDB: Optional txindex
├── llmq/                   # LLMQ DKG sessions, signatures
│
├── mempool.dat             # Unconfirmed transactions
├── peers.dat               # Known peer addresses
├── mncache.dat             # Masternode list cache
├── governance.dat          # Governance proposals/votes
├── sporks.dat              # Network feature flags
├── fee_estimates.dat       # Fee estimation data
├── banlist.dat             # Banned peers
├── netfulfilled.dat        # Request rate limiting
│
├── dash.conf               # Node configuration
├── wallet.dat              # Wallet (private keys)
├── settings.json           # GUI/runtime settings
│
├── debug.log               # Current log
├── debug.log.1             # Rotated log (older)
├── debug.log.2             # Even older
├── ...
└── dashd.pid               # Process ID
```

---

## For daino Development

### What You Need

For different stages of the indexer:

#### Stage 1: Basic Block Reader (Current)
- **Required**: `blk*.dat` files
- **Why**: Contains the raw block data to parse

#### Stage 2: Block Indexer
- **Required**: `blk*.dat` files
- **Optional**: `index/` directory (to understand Dash Core's indexing approach)
- **Why**: Learn how to map block hashes to file positions

#### Stage 3: Full Indexer
- **Study**: `chainstate/`, `evodb/`, `llmq/` directories
- **Why**: Understand how Dash Core stores state
- **Your own**: Build similar databases using LMDB/RocksDB

#### Stage 4: Real-time Sync
- **Study**: How `mempool.dat` and ZMQ work
- **Why**: Real-time block notifications

### File Sizes (Approximate)

- **blk*.dat**: ~128 MB each (compressed blocks)
- **rev*.dat**: Variable (undo data)
- **chainstate/**: 2-5 GB (UTXO set)
- **evodb/**: 500 MB - 2 GB (masternode data)
- **index/**: 5-10 GB (block index)
- **indexes/**: 20-50 GB (full txindex)

---

## Network-Specific Paths

### Mainnet
- Linux: `~/.dashcore/`
- macOS: `~/Library/Application Support/DashCore/`
- Windows: `%APPDATA%\DashCore\`

### Testnet
- Add `/testnet3/` subdirectory
- Example: `~/.dashcore/testnet3/blocks/`

### Regtest
- Add `/regtest/` subdirectory
- Example: `~/.dashcore/regtest/blocks/`

---

## Quick Commands

### Find block files
```bash
# Linux/macOS
ls ~/.dashcore/blocks/blk*.dat

# Docker container
ls /app/data/blocks/blk*.dat
```

### Check disk usage
```bash
# Total blocks directory size
du -sh ~/.dashcore/blocks/

# Individual file sizes
ls -lh ~/.dashcore/blocks/blk*.dat
```

### Count blocks in file
```bash
# This will be possible once daino implements block counting
./daino -c 99999999 ~/.dashcore/blocks/blk00000.dat | grep "Block" | wc -l
```

### Tail debug log
```bash
tail -f ~/.dashcore/debug.log
```

---

## References

- [Dash Core Data Directory Docs](https://docs.dash.org/en/stable/)
- [Bitcoin Core Data Directory](https://en.bitcoin.it/wiki/Data_directory) (similar structure)
- Dash Core Source: `src/init.cpp` (initialization code)
- Dash Core Source: `src/validation.cpp` (block validation)

---

**Last Updated**: January 1, 2026
