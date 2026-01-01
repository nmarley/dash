# Daino Architecture Design

**A Dash Blockchain Indexer Service**

---

**Date:** January 1, 2026
**Status:** Architecture & Planning Phase
**Repository:** Future separate repo (`daino`)
**Inspiration:** Zaino (Zcash indexer by Zingo Labs)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Context: Zaino Analysis](#context-zaino-analysis)
3. [Fork vs. Build Decision](#fork-vs-build-decision)
4. [Daino Architecture](#daino-architecture)
5. [Component Specifications](#component-specifications)
6. [Implementation Roadmap](#implementation-roadmap)
7. [Testing Strategy](#testing-strategy)
8. [Appendix: References](#appendix-references)

---

## Executive Summary

### What is Daino?

**Daino** is a planned blockchain indexer service for Dash, inspired by Zaino (the Zcash indexer). It will be a **separate repository** from librustdash, following the proven Zaino/librustzcash separation pattern.

### Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Repository Structure** | Separate repo from librustdash | Library vs. Application separation |
| **Implementation Approach** | Build from scratch using Zaino as blueprint | 70-80% rewrite needed if forked; Zcash-specific code |
| **Architecture Pattern** | Copy Zaino's workspace structure | Proven modular design |
| **Technology Stack** | Rust + tokio + tonic + jsonrpsee + LMDB | Reuse Zaino's stack where applicable |
| **Primitives Library** | Depends on librustdash | Dash block/tx parsing, X11 hashing, BLS |

### Project Scope

```
librustdash/              # Primitives library (this repo)
  ├── Block/Transaction parsing
  ├── Serialization/Deserialization
  ├── X11 hash computation
  ├── BLS signatures
  └── Core Dash types

daino/                    # Indexer service (NEW separate repo)
  ├── Depends on: librustdash
  ├── Block indexing
  ├── Masternode tracking
  ├── ChainLock tracking
  ├── Governance indexing
  ├── gRPC server
  └── Storage backends (LMDB/RocksDB)
```

---

## Context: Zaino Analysis

### What is Zaino?

Zaino is a Rust-based blockchain indexer for Zcash, part of the Z³ stack (Zebra validator + Zaino indexer + Zallet wallet). It provides:

- gRPC API for wallets and block explorers
- RPC compatibility with legacy Zcashd
- Direct state service integration with Zebra validator
- Modular backend system

**Key Stats:**
- 2,843 commits
- 21 contributors
- 99.2% Rust
- Apache-2.0 license

### Zaino Workspace Structure

```
zaino/
├── zaino-common         # Shared utilities
├── zaino-proto          # Protocol buffer definitions
├── zaino-fetch          # RPC client (jsonrpsee + chain logic)
├── zaino-state          # State management (backends/, chain_index/, local_cache/)
├── zaino-serve          # gRPC server (tonic)
├── zainod               # Main daemon executable
├── zaino-testutils      # Test helpers
├── zaino-testvectors    # Test data (real Zcash blocks)
└── integration-tests    # E2E tests
```

### Zaino Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                   Client Applications                    │
│         (wallets, explorers, mobile apps)               │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │ gRPC API
┌─────────────────────────────────────────────────────────┐
│                   zaino-serve                            │
│  - gRPC server (tonic)                                  │
│  - CompactTxStreamer service                            │
│  - Request validation                                   │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │
┌─────────────────────────────────────────────────────────┐
│                   zaino-state                            │
│  - State manager                                        │
│  - Configurable backends (backends/)                    │
│  - Chain indexing (chain_index/)                        │
│  - Local caching (local_cache/)                         │
└─────────────────────────────────────────────────────────┘
              ▲                          ▲
              │                          │
┌─────────────┴──────────┐   ┌──────────┴─────────────┐
│   zaino-fetch          │   │   librustzcash          │
│   (Data Sources)       │   │   (Primitives)         │
│  - Zebra RPC client    │   │  - Block parsing       │
│  - jsonrpsee           │   │  - Tx deserialization  │
│  - Mempool access      │   │  - Zcash crypto        │
└────────────────────────┘   └────────────────────────┘
```

### Zaino Technology Stack

**Zcash-Specific Dependencies (35% - 11 crates):**
- `librustzcash` family: zcash_address, zcash_keys, zcash_primitives, zcash_protocol, zcash_transparent
- `zebra-*` family: zebra-chain, zebra-state, zebra-rpc

**General-Purpose Dependencies (65% - 30+ crates):**
- **Runtime:** tokio (1.38), async-trait, futures, async-stream
- **RPC/Networking:** tonic, jsonrpsee, reqwest, tower, hyper, http
- **Serialization:** serde, serde_json, prost, toml
- **Storage:** lmdb, dashmap, crossbeam-channel
- **Cryptography:** sha2, blake2, base64, hex
- **CLI/Utils:** clap, tracing, chrono, thiserror
- **Testing:** portpicker, tempfile, proptest

### Key Zaino Design Patterns

#### 1. Configurable Backend Abstraction ⭐

```rust
// zaino-state/src/backends/
// Abstracts over data sources
trait Backend {
    fn get_block(&self, height: u32) -> Result<Block>;
    fn get_transaction(&self, txid: &[u8]) -> Result<Transaction>;
}

// Implementations:
// - Zebra ReadStateService (direct state access)
// - RPC client (backward compatibility)
```

**Why This Matters:** Allows mixing data sources (files, RPC, direct node integration) without changing indexer logic.

#### 2. Layered State Management

```
zaino-state/src/
├── backends/          # Data source abstraction
├── chain_index/       # Indexing logic (separate from storage)
├── local_cache/       # Performance layer
├── broadcast.rs       # Broadcasting functionality
├── stream.rs          # Stream processing
└── indexer.rs         # Core indexing operations
```

**Why This Matters:** Clean separation between data fetching, indexing, and storage.

#### 3. Dual Access Pattern

- **Library Mode:** Direct integration (zaino-state as library dependency)
- **Service Mode:** gRPC server (zainod daemon)

**Why This Matters:** Supports both embedded use (full node wallets) and remote clients.

#### 4. Comprehensive Testing

```
zaino-testutils/      # Test helpers
zaino-testvectors/    # Golden test data (real Zcash blocks)
integration-tests/    # E2E tests
```

---

## Fork vs. Build Decision

### Decision: **Build Daino from Scratch**

### Why NOT Fork Zaino

#### 1. Deep Zcash-Specific Integration (70-80% rewrite needed)

**Zebra Coupling:**
- `zaino-state` directly depends on `zebra-state::ReadStateService`
- Chain indexing assumes Zebra's internal state model
- Would require complete replacement of `backends/` implementation

**Transaction Model Mismatch:**
```rust
// Zcash model (Zaino assumes):
- Transparent UTXOs
- Shielded notes (Sapling/Orchard)
- Viewing keys, nullifiers, diversifiers
- zk-SNARK proofs

// Dash model (Daino needs):
- Classical UTXOs
- Special Transactions:
  - ProRegTx (masternode registration)
  - ProUpServTx (masternode updates)
  - ProUpRegTx (operator key updates)
  - ProUpRevTx (masternode revocation)
- Masternode list tracking
- LLMQ quorum tracking
- ChainLocks (block finality)
- InstantSend locks
- Governance objects
```

**Protocol Buffers are Zcash-Specific:**
- CompactTxStreamer gRPC service
- CompactBlock format (Zcash light client protocol)
- Viewing keys and diversifiers in API
- Complete rewrite required for Dash

#### 2. Cryptography Differences

```rust
// Zcash:
librustzcash primitives (zk-SNARKs, Jubjub curves, etc.)

// Dash:
BLS signatures (masternode quorums)
X11 hashing (PoW)
```

#### 3. Effort Analysis

| Component | Fork Approach | Build Approach |
|-----------|---------------|----------------|
| zaino-proto | 100% rewrite | Write from scratch |
| zaino-state | 70% rewrite | Copy architecture, write new |
| zaino-fetch | 50% rewrite | Copy RPC pattern, adapt |
| zaino-serve | 30% rewrite | Copy tonic scaffold, adapt |
| zaino-common | 20% rewrite | Copy utilities |
| **Total Effort** | **60-70% new code** | **Same effort, cleaner result** |

**Conclusion:** Forking provides minimal benefit while adding complexity of removing Zcash assumptions.

### What TO Copy from Zaino

#### High-Value Patterns (Copy Architecture, Not Code)

✅ **Workspace Structure**
- 9-crate modular layout
- Clear separation of concerns
- Cargo.toml organization

✅ **Backend Abstraction Pattern** (zaino-state/backends/)
- Pluggable data sources
- Single interface for multiple backends

✅ **RPC Client Pattern** (zaino-fetch)
- jsonrpsee usage
- Error handling
- Async patterns

✅ **gRPC Server Scaffold** (zaino-serve)
- tonic server setup
- Request validation
- Error mapping

✅ **Testing Infrastructure**
- Test utilities organization
- Golden test vectors
- Integration test patterns

#### Medium-Value Code (Copy with Modifications)

🔶 **Common Utilities** (zaino-common)
- Logging setup
- Configuration parsing
- Generic helpers

🔶 **Daemon Boilerplate** (zainod)
- CLI argument parsing (clap)
- Service initialization
- Shutdown handling

#### Low-Value (Don't Copy)

❌ **Protocol Definitions** (zaino-proto) - 100% Zcash-specific
❌ **Indexing Logic** (zaino-state/chain_index/) - Zcash transactions
❌ **Zebra Integration** - No Dash equivalent

---

## Daino Architecture

### Design Principles

1. **Modular Workspace** - Separate crates for each concern
2. **Backend Abstraction** - Pluggable data sources (RPC, files, ZMQ)
3. **Dual Deployment** - Library mode + Service mode
4. **Testability** - Mock backends, golden tests
5. **Dash-Native** - Built on librustdash primitives

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                   Client Applications                    │
│    (wallets, explorers, governance dashboards)          │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │ gRPC API
┌─────────────────────────────────────────────────────────┐
│                   daino-serve                            │
│  - gRPC server (tonic)                                  │
│  - Dash RPC methods:                                    │
│    - GetBlock, GetTransaction                           │
│    - GetAddressBalance, GetAddressUTXOs                 │
│    - GetMasternodeList, GetQuorumInfo                   │
│    - GetGovernanceObjects, GetChainLocks                │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │
┌─────────────────────────────────────────────────────────┐
│                   daino-state                            │
│  - State manager                                        │
│  - Configurable backends (backends/)                    │
│  - Dash-specific indexing (chain_index/):              │
│    - Masternode tracker (ProRegTx, ProUpServTx, etc.)  │
│    - ChainLock tracker                                  │
│    - InstantSend lock tracker                           │
│    - Governance indexer                                 │
│    - LLMQ quorum tracker                                │
│  - Local caching (local_cache/)                         │
│  - Storage abstraction (LMDB/RocksDB)                  │
└─────────────────────────────────────────────────────────┘
              ▲                          ▲
              │                          │
┌─────────────┴──────────┐   ┌──────────┴─────────────┐
│   daino-fetch          │   │   librustdash          │
│   (Data Sources)       │   │   (Primitives)         │
│  - dashd RPC client    │   │  - Block parsing       │
│  - jsonrpsee           │   │  - Tx deserialization  │
│  - Block file reader   │   │  - X11 hashing         │
│  - ZMQ subscriber      │   │  - BLS signatures      │
│  - Mempool access      │   │  - Special tx payloads │
└────────────────────────┘   └────────────────────────┘
```

### Workspace Structure

```
daino/
├── Cargo.toml                    # Workspace manifest
├── README.md
├── LICENSE
├── docs/
│   ├── architecture.md
│   ├── rpc_api.md
│   └── deployment.md
│
├── daino-common/                 # Shared utilities
│   ├── src/
│   │   ├── lib.rs
│   │   ├── config.rs             # Configuration types
│   │   ├── error.rs              # Common error types
│   │   └── utils.rs              # Helper functions
│   └── Cargo.toml
│
├── daino-proto/                  # Protocol buffer definitions
│   ├── proto/
│   │   └── daino.proto           # Dash-specific gRPC service
│   ├── src/
│   │   └── lib.rs                # Generated code
│   ├── build.rs                  # Prost/tonic build script
│   └── Cargo.toml
│
├── daino-fetch/                  # Data fetching layer
│   ├── src/
│   │   ├── lib.rs
│   │   ├── jsonrpsee.rs          # dashd RPC client
│   │   ├── chain.rs              # Chain data fetching
│   │   ├── mempool.rs            # Mempool access
│   │   └── error.rs
│   └── Cargo.toml
│
├── daino-state/                  # State management + indexing
│   ├── src/
│   │   ├── lib.rs
│   │   ├── backends/             # Data source abstraction
│   │   │   ├── mod.rs
│   │   │   ├── dashd_rpc.rs      # JSON-RPC backend
│   │   │   ├── block_files.rs    # blk*.dat reader
│   │   │   └── zmq.rs            # ZMQ stream
│   │   ├── chain_index/          # Dash-specific indexing
│   │   │   ├── mod.rs
│   │   │   ├── masternode.rs     # MN tracking
│   │   │   ├── chainlock.rs      # ChainLock tracking
│   │   │   ├── instantsend.rs    # IS lock tracking
│   │   │   ├── governance.rs     # Governance objects
│   │   │   └── quorum.rs         # LLMQ quorum tracking
│   │   ├── local_cache/          # Caching layer
│   │   │   ├── mod.rs
│   │   │   └── lru.rs
│   │   ├── storage/              # Storage backends
│   │   │   ├── mod.rs
│   │   │   ├── lmdb.rs           # LMDB implementation
│   │   │   └── rocksdb.rs        # RocksDB implementation
│   │   ├── indexer.rs            # Core indexer
│   │   ├── config.rs
│   │   ├── status.rs
│   │   └── error.rs
│   └── Cargo.toml
│
├── daino-serve/                  # gRPC server
│   ├── src/
│   │   ├── lib.rs
│   │   ├── server.rs             # Tonic server setup
│   │   ├── handlers/             # RPC method handlers
│   │   │   ├── mod.rs
│   │   │   ├── blocks.rs
│   │   │   ├── transactions.rs
│   │   │   ├── addresses.rs
│   │   │   ├── masternodes.rs
│   │   │   └── governance.rs
│   │   └── error.rs
│   └── Cargo.toml
│
├── dainod/                       # Main executable
│   ├── src/
│   │   └── main.rs               # CLI + daemon
│   └── Cargo.toml
│
├── daino-testutils/              # Testing utilities
│   ├── src/
│   │   ├── lib.rs
│   │   ├── mock_backend.rs       # Mock data sources
│   │   └── fixtures.rs           # Test data generation
│   └── Cargo.toml
│
├── daino-testvectors/            # Golden test data
│   ├── data/
│   │   ├── mainnet/              # Real mainnet blocks
│   │   └── testnet/              # Real testnet blocks
│   ├── src/
│   │   └── lib.rs                # Test vector loading
│   └── Cargo.toml
│
└── integration-tests/            # E2E tests
    ├── tests/
    │   ├── indexer_tests.rs
    │   ├── rpc_tests.rs
    │   └── golden_tests.rs
    └── Cargo.toml
```

### Technology Stack

```toml
[workspace.dependencies]
# Dash primitives
librustdash = { version = "0.1", path = "../librustdash" }

# Async runtime
tokio = { version = "1.38", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# RPC & Networking
tonic = { version = "0.12", features = ["tls"] }
tonic-build = "0.12"
prost = "0.13"
jsonrpsee = { version = "0.24", features = ["client", "server"] }
tower = "0.4"
hyper = "1.0"

# Storage
lmdb = "0.8"
rocksdb = { version = "0.21", optional = true }
dashmap = "6.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
byteorder = "1.5"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI
clap = { version = "4.5", features = ["derive"] }

# Utilities
hex = "0.4"
base64 = "0.22"
chrono = "0.4"

# Testing
tempfile = "3.0"
proptest = "1.5"
portpicker = "0.1"

# NOT needed (Zcash-only):
# zebra-*
# zcash_*
# zingolib
```

---

## Component Specifications

### daino-common

**Purpose:** Shared types and utilities

**Key Exports:**
```rust
// Configuration
pub struct DainoConfig {
    pub network: Network,
    pub data_dir: PathBuf,
    pub backend: BackendConfig,
    pub storage: StorageConfig,
    pub server: ServerConfig,
}

// Network
pub enum Network {
    Mainnet,
    Testnet,
    Devnet,
    Regtest,
}

// Error types
pub enum DainoError {
    Io(io::Error),
    Serialization(String),
    Backend(String),
    Storage(String),
    Rpc(String),
}
```

---

### daino-proto

**Purpose:** gRPC protocol definitions

**API Design:**
```protobuf
syntax = "proto3";

package daino.v1;

service DainoService {
  // Block queries
  rpc GetBlock(GetBlockRequest) returns (Block);
  rpc GetBlockByHash(GetBlockByHashRequest) returns (Block);
  rpc GetBlockRange(GetBlockRangeRequest) returns (stream Block);

  // Transaction queries
  rpc GetTransaction(GetTransactionRequest) returns (Transaction);
  rpc GetRawTransaction(GetTransactionRequest) returns (RawTransaction);

  // Address queries
  rpc GetAddressBalance(GetAddressBalanceRequest) returns (AddressBalance);
  rpc GetAddressUtxos(GetAddressUtxosRequest) returns (AddressUtxos);
  rpc GetAddressHistory(GetAddressHistoryRequest) returns (stream TxRef);

  // Masternode queries
  rpc GetMasternodeList(GetMasternodeListRequest) returns (MasternodeList);
  rpc GetMasternode(GetMasternodeRequest) returns (MasternodeInfo);
  rpc GetQuorumInfo(GetQuorumInfoRequest) returns (QuorumInfo);

  // Governance queries
  rpc GetGovernanceObjects(GetGovernanceObjectsRequest) returns (stream GovernanceObject);
  rpc GetGovernanceVotes(GetGovernanceVotesRequest) returns (stream GovernanceVote);

  // ChainLock queries
  rpc GetChainLock(GetChainLockRequest) returns (ChainLock);
  rpc GetLatestChainLock(Empty) returns (ChainLock);

  // InstantSend queries
  rpc GetInstantSendLock(GetInstantSendLockRequest) returns (InstantSendLock);

  // Mempool
  rpc GetMempoolInfo(Empty) returns (MempoolInfo);
  rpc GetRawMempool(Empty) returns (stream TxId);

  // Subscription streams
  rpc SubscribeBlocks(Empty) returns (stream Block);
  rpc SubscribeTransactions(SubscribeTransactionsRequest) returns (stream Transaction);
  rpc SubscribeChainLocks(Empty) returns (stream ChainLock);
}

message Block {
  uint32 height = 1;
  bytes hash = 2;
  BlockHeader header = 3;
  repeated Transaction transactions = 4;
}

message MasternodeInfo {
  bytes protx_hash = 1;
  string ip_address = 2;
  uint32 port = 3;
  bytes operator_pubkey = 4;
  bytes voting_pubkey = 5;
  MasternodeState state = 6;
}

// ... other message types
```

---

### daino-fetch

**Purpose:** Fetch data from dashd or block files

**Key Traits:**
```rust
/// Abstraction over block data sources
#[async_trait]
pub trait BlockSource: Send + Sync {
    /// Fetch a block by height
    async fn get_block(&self, height: u32) -> Result<Block>;

    /// Fetch a block by hash
    async fn get_block_by_hash(&self, hash: &[u8; 32]) -> Result<Block>;

    /// Fetch current chain tip
    async fn get_chain_tip(&self) -> Result<(u32, [u8; 32])>;

    /// Fetch mempool transactions
    async fn get_mempool_txs(&self) -> Result<Vec<Transaction>>;
}

/// dashd RPC implementation
pub struct DashdRpcClient {
    client: jsonrpsee::http_client::HttpClient,
    url: String,
}

impl DashdRpcClient {
    pub async fn new(url: String, auth: Option<(String, String)>) -> Result<Self>;
}

#[async_trait]
impl BlockSource for DashdRpcClient {
    async fn get_block(&self, height: u32) -> Result<Block> {
        // 1. Get block hash by height
        let hash: String = self.client
            .request("getblockhash", rpc_params![height])
            .await?;

        // 2. Get block by hash
        let block_hex: String = self.client
            .request("getblock", rpc_params![hash, 0])
            .await?;

        // 3. Deserialize using librustdash
        let block_bytes = hex::decode(block_hex)?;
        let block = Block::deserialize(&block_bytes)?;

        Ok(block)
    }

    // ... other methods
}
```

**Block File Reader:**
```rust
/// Reads blocks from blk*.dat files
pub struct BlockFileReader {
    block_dir: PathBuf,
    network: Network,
    current_file: u32,
    position: u64,
}

impl BlockFileReader {
    pub fn new(block_dir: PathBuf, network: Network) -> Result<Self>;

    /// Read next block from files
    pub fn read_next_block(&mut self) -> Result<Option<Block>> {
        // Read magic bytes (0xBF0C6BBD for mainnet)
        // Read size (4 bytes)
        // Read block data
        // Deserialize with librustdash
    }

    /// Seek to specific position
    pub fn seek(&mut self, file: u32, offset: u64) -> Result<()>;
}
```

---

### daino-state

**Purpose:** State management, indexing, and storage

**Backend Abstraction:**
```rust
/// Backend trait (inspired by zaino-state/backends/)
#[async_trait]
pub trait Backend: Send + Sync {
    /// Fetch block by height
    async fn fetch_block(&self, height: u32) -> Result<Block>;

    /// Fetch transaction by txid
    async fn fetch_transaction(&self, txid: &[u8; 32]) -> Result<Transaction>;

    /// Fetch masternode list at height
    async fn fetch_masternode_list(&self, height: u32) -> Result<Vec<MasternodeInfo>>;

    /// Fetch current chain tip
    async fn fetch_chain_tip(&self) -> Result<(u32, [u8; 32])>;
}

/// RPC backend
pub struct RpcBackend {
    client: DashdRpcClient,
}

/// Block file backend
pub struct BlockFileBackend {
    reader: BlockFileReader,
    index: HashMap<u32, (u32, u64)>, // height -> (file, offset)
}

/// ZMQ backend (future)
pub struct ZmqBackend {
    zmq_ctx: zmq::Context,
    block_socket: zmq::Socket,
}
```

**Indexing Traits:**
```rust
/// Masternode index
#[async_trait]
pub trait MasternodeIndex: Send + Sync {
    /// Index a masternode registration
    async fn index_registration(
        &mut self,
        height: u32,
        protx_hash: &[u8; 32],
        protx: &ProRegTx,
    ) -> Result<()>;

    /// Index a masternode update
    async fn index_update(
        &mut self,
        height: u32,
        protx_hash: &[u8; 32],
        update: MnUpdate,
    ) -> Result<()>;

    /// Get masternode by protx hash
    async fn get_masternode(&self, protx_hash: &[u8; 32]) -> Result<Option<MasternodeInfo>>;

    /// Get all active masternodes at height
    async fn get_active_masternodes(&self, height: u32) -> Result<Vec<MasternodeInfo>>;
}

/// ChainLock index
#[async_trait]
pub trait ChainLockIndex: Send + Sync {
    /// Index a ChainLock
    async fn index_chainlock(
        &mut self,
        height: u32,
        block_hash: &[u8; 32],
        signature: &[u8; 96],
    ) -> Result<()>;

    /// Get ChainLock for block
    async fn get_chainlock(&self, block_hash: &[u8; 32]) -> Result<Option<ChainLock>>;

    /// Get latest ChainLock
    async fn get_latest_chainlock(&self) -> Result<Option<ChainLock>>;
}

/// Governance index
#[async_trait]
pub trait GovernanceIndex: Send + Sync {
    /// Index a governance object
    async fn index_governance_object(
        &mut self,
        hash: &[u8; 32],
        obj: &GovernanceObject,
    ) -> Result<()>;

    /// Get governance objects by type
    async fn get_governance_objects(
        &self,
        obj_type: GovernanceObjectType,
    ) -> Result<Vec<GovernanceObject>>;
}
```

**Storage Implementation:**
```rust
/// LMDB storage backend
pub struct LmdbStorage {
    env: lmdb::Environment,

    // Databases
    blocks_db: lmdb::Database,          // height -> Block
    block_hash_db: lmdb::Database,      // hash -> height
    txs_db: lmdb::Database,             // txid -> Transaction
    tx_height_db: lmdb::Database,       // txid -> height

    // Address indexing
    address_utxos_db: lmdb::Database,   // address -> [UTXO]
    address_history_db: lmdb::Database, // address -> [TxRef]

    // Dash-specific
    masternodes_db: lmdb::Database,     // protx_hash -> MasternodeInfo
    mn_by_height_db: lmdb::Database,    // height -> [protx_hash]
    chainlocks_db: lmdb::Database,      // block_hash -> ChainLock
    governance_db: lmdb::Database,      // gov_hash -> GovernanceObject
    quorums_db: lmdb::Database,         // (llmqType, quorumHash) -> QuorumInfo
}

impl LmdbStorage {
    pub fn new(path: PathBuf) -> Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_dbs(20)
            .set_map_size(500 * 1024 * 1024 * 1024) // 500 GB
            .open(&path)?;

        // Create databases...

        Ok(Self { /* ... */ })
    }
}

// Implement BlockStore, TxStore, MasternodeIndex, ChainLockIndex, etc.
```

**Core Indexer:**
```rust
/// Main indexer
pub struct Indexer<B: Backend> {
    backend: Arc<B>,
    storage: Arc<Mutex<LmdbStorage>>,
    config: IndexerConfig,
}

impl<B: Backend> Indexer<B> {
    pub async fn new(backend: B, storage: LmdbStorage, config: IndexerConfig) -> Self {
        Self {
            backend: Arc::new(backend),
            storage: Arc::new(Mutex::new(storage)),
            config,
        }
    }

    /// Index blocks from start_height to end_height
    pub async fn index_range(&self, start: u32, end: u32) -> Result<()> {
        for height in start..=end {
            let block = self.backend.fetch_block(height).await?;
            self.index_block(height, block).await?;

            if height % 1000 == 0 {
                info!("Indexed {} blocks", height);
            }
        }
        Ok(())
    }

    /// Index a single block
    async fn index_block(&self, height: u32, block: Block) -> Result<()> {
        let mut storage = self.storage.lock().await;

        // Start transaction
        storage.begin_transaction()?;

        // Store block
        storage.put_block(height, &block)?;

        // Index transactions
        for tx in &block.transactions {
            let txid = tx.txid()?;
            storage.put_transaction(&txid, tx, height)?;

            // Index addresses (extract from outputs)
            self.index_addresses(tx, height, &mut storage)?;

            // Index special transactions
            if tx.is_special_transaction() {
                self.index_special_tx(tx, height, &mut storage)?;
            }
        }

        // Commit transaction
        storage.commit()?;

        Ok(())
    }

    /// Index special transactions (masternodes, governance, etc.)
    fn index_special_tx(
        &self,
        tx: &Transaction,
        height: u32,
        storage: &mut LmdbStorage,
    ) -> Result<()> {
        use librustdash::DashTxType;

        match tx.tx_type {
            DashTxType::ProviderRegister => {
                let protx = ProRegTx::deserialize(&tx.extra_payload)?;
                storage.index_registration(height, &tx.txid()?, &protx).await?;
            }
            DashTxType::ProviderUpdateService => {
                let update = ProUpServTx::deserialize(&tx.extra_payload)?;
                storage.index_update(height, &update.protx_hash, MnUpdate::Service(update)).await?;
            }
            // ... other special tx types
            _ => {}
        }

        Ok(())
    }
}
```

---

### daino-serve

**Purpose:** gRPC server implementation

**Server Setup:**
```rust
use tonic::{transport::Server, Request, Response, Status};
use daino_proto::daino_service_server::{DainoService, DainoServiceServer};

pub struct DainoServiceImpl {
    state: Arc<DainoState>,
}

#[tonic::async_trait]
impl DainoService for DainoServiceImpl {
    async fn get_block(
        &self,
        request: Request<GetBlockRequest>,
    ) -> Result<Response<Block>, Status> {
        let height = request.into_inner().height;

        let block = self.state
            .get_block(height)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Block not found"))?;

        Ok(Response::new(block.into()))
    }

    async fn get_masternode_list(
        &self,
        request: Request<GetMasternodeListRequest>,
    ) -> Result<Response<MasternodeList>, Status> {
        let req = request.into_inner();
        let height = req.height.unwrap_or_else(|| self.state.get_tip_height());

        let masternodes = self.state
            .get_active_masternodes(height)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(MasternodeList {
            height,
            masternodes: masternodes.into_iter().map(Into::into).collect(),
        }))
    }

    // ... implement all RPC methods
}

pub async fn serve(config: ServerConfig, state: Arc<DainoState>) -> Result<()> {
    let addr = format!("{}:{}", config.host, config.port).parse()?;
    let service = DainoServiceImpl { state };

    info!("Starting Daino gRPC server on {}", addr);

    Server::builder()
        .add_service(DainoServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
```

---

### dainod

**Purpose:** Main daemon executable

**CLI:**
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dainod")]
#[command(about = "Dash blockchain indexer daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the indexer daemon
    Start {
        #[arg(long)]
        network: Option<String>,

        #[arg(long)]
        data_dir: Option<PathBuf>,

        #[arg(long)]
        dashd_url: Option<String>,
    },

    /// Index historical blocks from files
    Index {
        #[arg(long)]
        blocks_dir: PathBuf,

        #[arg(long)]
        start_height: Option<u32>,

        #[arg(long)]
        end_height: Option<u32>,
    },

    /// Query indexed data
    Query {
        #[command(subcommand)]
        query: QueryCommands,
    },
}

#[derive(Subcommand)]
enum QueryCommands {
    Block { height: u32 },
    Transaction { txid: String },
    Address { address: String },
    Masternode { protx_hash: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = load_config(cli.config.as_ref())?;

    match cli.command {
        Commands::Start { network, data_dir, dashd_url } => {
            let config = merge_config(config, network, data_dir, dashd_url);
            run_daemon(config).await?;
        }
        Commands::Index { blocks_dir, start_height, end_height } => {
            run_indexer(config, blocks_dir, start_height, end_height).await?;
        }
        Commands::Query { query } => {
            run_query(config, query).await?;
        }
    }

    Ok(())
}

async fn run_daemon(config: DainoConfig) -> Result<()> {
    info!("Starting Daino daemon");

    // Initialize backend
    let backend = RpcBackend::new(config.backend).await?;

    // Initialize storage
    let storage = LmdbStorage::new(config.data_dir.join("index"))?;

    // Initialize indexer
    let indexer = Indexer::new(backend, storage, config.indexer).await;

    // Initialize state (shared between indexer and server)
    let state = Arc::new(DainoState::new(indexer));

    // Start gRPC server
    let server_handle = tokio::spawn(serve(config.server, state.clone()));

    // Start indexing loop
    let indexer_handle = tokio::spawn(async move {
        loop {
            if let Err(e) = state.sync_to_tip().await {
                error!("Indexing error: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received");
        }
        _ = server_handle => {}
        _ = indexer_handle => {}
    }

    Ok(())
}
```

---

## Implementation Roadmap

### Phase 1: Primitives (2 weeks) ✅ In Progress

**Goal:** Complete librustdash foundation

**Tasks:**
- ✅ Block/Transaction parsing (mostly done)
- ✅ Serialization/Deserialization (mostly done)
- 🔲 X11 hash computation (see HASH_INTEGRATION_PLAN.md)
- 🔲 BLS signature support
- 🔲 All special transaction payloads (ProRegTx, ProUpServTx, etc.)

**Deliverable:** librustdash v0.1.0

---

### Phase 2: Workspace Setup (1 week)

**Goal:** Create daino repository with workspace structure

**Tasks:**
1. Create new repo: `daino/`
2. Copy workspace structure from Zaino
3. Set up Cargo.toml with dependencies
4. Add librustdash dependency
5. Basic README and documentation

**Deliverable:** Buildable workspace skeleton

---

### Phase 3: Fetch Layer (1 week)

**Goal:** Implement daino-fetch

**Tasks:**
1. dashd RPC client (jsonrpsee)
   - getblock, getblockhash
   - getrawtransaction
   - getmempoolinfo, getrawmempool
2. Block file reader
   - Read blk*.dat files
   - Parse using librustdash
3. Testing
   - Unit tests
   - Integration tests with dashd regtest

**Deliverable:** Working data fetching layer

---

### Phase 4: State Layer Foundation (2 weeks)

**Goal:** Implement daino-state basics

**Tasks:**
1. Backend abstraction
   - RpcBackend implementation
   - BlockFileBackend implementation
2. LMDB storage
   - Database setup
   - Block storage
   - Transaction storage
3. Basic indexer
   - Index blocks sequentially
   - Store blocks and transactions

**Deliverable:** Basic block indexer

---

### Phase 5: Dash-Specific Indexing (2-3 weeks)

**Goal:** Add masternode/ChainLock/governance indexing

**Tasks:**
1. Masternode tracking
   - ProRegTx indexing
   - ProUpServTx indexing
   - ProUpRegTx indexing
   - ProUpRevTx indexing
   - Masternode list by height
2. ChainLock tracking
   - Index ChainLocks
   - Latest ChainLock queries
3. Governance indexing
   - Governance object storage
   - Governance votes
4. LLMQ quorum tracking (optional, future)

**Deliverable:** Full Dash feature indexing

---

### Phase 6: gRPC Service (1 week)

**Goal:** Implement daino-serve and daino-proto

**Tasks:**
1. Define protocol buffers
   - Service definition
   - Message types
2. Implement gRPC server
   - Block queries
   - Transaction queries
   - Address queries
   - Masternode queries
   - Governance queries
3. Error handling
4. Request validation

**Deliverable:** Working gRPC API

---

### Phase 7: Daemon (1 week)

**Goal:** Implement dainod executable

**Tasks:**
1. CLI with clap
   - start, index, query commands
2. Configuration loading
3. Service initialization
4. Continuous sync loop
5. Graceful shutdown

**Deliverable:** Production-ready daemon

---

### Phase 8: Testing & Documentation (1-2 weeks)

**Goal:** Comprehensive testing and docs

**Tasks:**
1. Unit tests for all components
2. Integration tests
3. Golden tests with real Dash blocks
4. Performance benchmarks
5. API documentation
6. Deployment guide
7. Developer guide

**Deliverable:** Release-ready v0.1.0

---

### Phase 9: Advanced Features (Future)

**Goal:** Production hardening

**Tasks:**
1. ZMQ backend (real-time block streaming)
2. RocksDB backend (alternative to LMDB)
3. Reorg handling
4. InstantSend lock tracking
5. Metrics/monitoring (Prometheus)
6. Docker deployment
7. Kubernetes deployment
8. High-availability setup

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use daino_testutils::*;

    #[tokio::test]
    async fn test_rpc_backend_fetch_block() {
        let backend = RpcBackend::new_mock();
        let block = backend.fetch_block(0).await.unwrap();
        assert_eq!(block.transactions.len(), 1); // Genesis block
    }

    #[tokio::test]
    async fn test_lmdb_storage_put_get_block() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut storage = LmdbStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let block = create_test_block();
        storage.put_block(0, &block).await.unwrap();

        let retrieved = storage.get_block(0).await.unwrap().unwrap();
        assert_eq!(retrieved.hash(), block.hash());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_indexer_end_to_end() {
    // Start regtest dashd
    let dashd = start_regtest_dashd().await;

    // Generate some blocks
    dashd.generate_blocks(100).await;

    // Create indexer
    let backend = RpcBackend::new(dashd.rpc_url()).await.unwrap();
    let storage = LmdbStorage::new_temp().unwrap();
    let indexer = Indexer::new(backend, storage, IndexerConfig::default()).await;

    // Index blocks
    indexer.index_range(0, 100).await.unwrap();

    // Verify
    let block = indexer.storage.get_block(50).await.unwrap().unwrap();
    assert_eq!(block.transactions.len(), 1);
}
```

### Golden Tests

```rust
#[tokio::test]
async fn test_mainnet_genesis_block() {
    let block_data = include_bytes!("../daino-testvectors/data/mainnet/block_0.dat");
    let block = Block::deserialize(block_data).unwrap();

    assert_eq!(
        hex::encode(block.hash()),
        "00000ffd590b1485b3caadc19b22e6379c733355108f107a430458cdf3407ab6"
    );
}

#[tokio::test]
async fn test_index_first_1000_blocks() {
    let blocks_dir = PathBuf::from("daino-testvectors/data/mainnet");
    let backend = BlockFileBackend::new(blocks_dir, Network::Mainnet).unwrap();
    let storage = LmdbStorage::new_temp().unwrap();

    let indexer = Indexer::new(backend, storage, IndexerConfig::default()).await;
    indexer.index_range(0, 1000).await.unwrap();

    // Verify all blocks indexed correctly
    for height in 0..=1000 {
        let block = indexer.storage.get_block(height).await.unwrap();
        assert!(block.is_some());
    }
}
```

### Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_block_serialization_roundtrip(
        version in any::<u32>(),
        timestamp in any::<u32>(),
    ) {
        let block = create_arbitrary_block(version, timestamp);
        let serialized = block.serialize().unwrap();
        let deserialized = Block::deserialize(&serialized).unwrap();
        prop_assert_eq!(block, deserialized);
    }
}
```

---

## Appendix: References

### Zaino Documentation
- [Zaino GitHub Repository](https://github.com/zingolabs/zaino)
- [Zaino Workspace Documentation](https://zingolabs.org/zaino/)
- [Zaino Internal Specification](https://github.com/zingolabs/zaino/blob/dev/docs/internal_spec.md)
- [Zaino Dev Branch](https://github.com/zingolabs/zaino/tree/dev)

### Zcash Community
- [Zaino Read State Service PR](https://github.com/zingolabs/zaino/pull/140)
- [Zaino Respecification Discussion](https://forum.zcashcommunity.com/t/zaino-respecification-no-delays/51319)
- [Z³ Stack Announcement](https://x.com/ZecHub/status/1947645593339285803)

### Dash Resources
- [Dash Core GitHub](https://github.com/dashpay/dash)
- [Dash Documentation](https://docs.dash.org/)
- [Dash Developer Guide](https://dashcore.readme.io/)

### Related librustdash Documentation
- `ARCHITECTURE.md` - librustdash architecture
- `INDEXER_ARCHITECTURE.md` - Original indexer design (pre-Zaino analysis)
- `HASH_INTEGRATION_PLAN.md` - X11 hashing implementation plan
- `X11_RUST_PORT_ANALYSIS.md` - X11 algorithm analysis

---

## Change Log

**January 1, 2026**
- Initial architecture document
- Zaino analysis (dev branch)
- Fork vs. build decision
- Complete Daino architecture specification
- Implementation roadmap

---

**End of Document**
