# Dash Indexer Service - Design Document

## Vision

Build a modular, high-performance indexer service for Dash, inspired by Zcash's Zaino. This service will separate indexing concerns from consensus validation, enabling better performance, clearer architecture, and future Rust migration of Dash Core components.

## Goals

1. **Separation of Concerns**: Decouple indexing from consensus validation
2. **Performance**: Reduce dashd resource usage for nodes that don't need full indexing
3. **Modern API**: Provide gRPC and JSON-RPC interfaces for ecosystem tools
4. **Foundation**: Prove the modular architecture pattern for future Dash Core modernization
5. **Ecosystem Enablement**: Better support for explorers, wallets, and analytics

## Non-Goals (v1)

- Replace dashd consensus/validation
- Wallet functionality
- Full API compatibility with dashd (backward compat is temporary bridge)
- Platform/Evolution integration (future phase)

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                  dash-indexer                       │
│                                                     │
│  ┌──────────────┐      ┌─────────────────────┐    │
│  │ ZMQ Consumer │─────▶│  Block Parser       │    │
│  │ (Real-time)  │      │  (Dash TX types)    │    │
│  └──────────────┘      └─────────────────────┘    │
│                               │                     │
│  ┌──────────────┐             ▼                     │
│  │ File Reader  │      ┌─────────────────────┐    │
│  │ (Bootstrap)  │─────▶│  Index Manager      │    │
│  └──────────────┘      └─────────────────────┘    │
│                               │                     │
│                               ▼                     │
│                        ┌─────────────┐             │
│                        │ LMDB Store  │             │
│                        └─────────────┘             │
│                               │                     │
│                               ▼                     │
│  ┌──────────────────────────────────────────────┐  │
│  │            API Layer                         │  │
│  │  ┌────────────┐  ┌──────────────┐          │  │
│  │  │ gRPC       │  │ JSON-RPC     │          │  │
│  │  │ (Primary)  │  │ (Compat)     │          │  │
│  │  └────────────┘  └──────────────┘          │  │
│  └──────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
         ▲                           │
         │ ZMQ notifications         │ Queries
         │                           ▼
    ┌────────┐              ┌──────────────────┐
    │ dashd  │              │ Client Apps      │
    │        │              │ (explorers,      │
    └────────┘              │  wallets, etc)   │
                            └──────────────────┘
```

## Data Flow

### 1. Bootstrap (Initial Sync)

```
dashd block files (blk*.dat)
    │
    ├─▶ Read blocks sequentially
    │   Parse headers + transactions
    │   Extract index data
    │   Store in LMDB
    │
    └─▶ Parallel processing (multiple blk files)
```

**Why direct file reading?**
- 10-100x faster than RPC/ZMQ replay
- Can process multiple files in parallel
- No impact on running dashd
- Zaino uses direct Zebra state access for same reason

### 2. Real-time Updates (Staying in Sync)

```
dashd ZMQ notifications
    │
    ├─▶ hashblock topic (new blocks)
    ├─▶ rawtx topic (mempool)
    │
    └─▶ Parse → Index → Store
```

**Why ZMQ?**
- Low latency (milliseconds)
- No polling overhead
- Clean event model
- Already supported by dashd

## Technology Stack

Following Zaino's proven approach:

### Core Infrastructure
- **Language**: Rust (2021 edition)
- **Async Runtime**: Tokio (full features)
- **Logging**: tracing + tracing-subscriber

### Storage
- **Primary**: LMDB (Lightning Memory-Mapped Database)
  - Used by Zaino
  - Fast, ACID-compliant
  - Zero-copy reads
  - Efficient for read-heavy indexing workloads

### APIs
- **gRPC**: Tonic + Prost (primary interface)
- **JSON-RPC**: jsonrpsee (backward compatibility)

### Dash Integration
- **Block Parsing**: Custom (based on Bitcoin format + Dash extensions)
- **ZMQ**: tokio-zmq or zmq crate
- **RPC Fallback**: bitcoincore-rpc (works with dashd)

## Dash-Specific Considerations

### Transaction Types

Dash extends Bitcoin with special transaction types (from `primitives/transaction.h:24-36`):

```rust
#[derive(Debug, Clone, Copy)]
pub enum DashTxType {
    Normal = 0,
    ProviderRegister = 1,        // Masternode registration
    ProviderUpdateService = 2,    // Masternode service update
    ProviderUpdateRegistrar = 3,  // Masternode registrar update
    ProviderUpdateRevoke = 4,     // Masternode revocation
    Coinbase = 5,                 // Coinbase with masternode payments
    QuorumCommitment = 6,         // LLMQ commitment
    MnhfSignal = 7,               // Masternode hard fork signal
    AssetLock = 8,                // Platform asset lock
    AssetUnlock = 9,              // Platform asset unlock
}
```

### Serialization Format

Dash combines version and type in a single 32-bit field:
```
n32bitVersion = nVersion | (nType << 16)
```

Special transactions (version >= 3, type != 0) have `vExtraPayload`.

### Indexes to Support

**Phase 1 (Prototype):**
- Transaction index (txid → block, position)

**Phase 2:**
- Address index (address → UTXOs)
- Spent index (outpoint → spending tx)
- Timestamp index (time → transactions)

**Phase 3 (Dash-specific):**
- Masternode payment tracking
- Governance proposal index
- InstantSend lock index
- ChainLock metadata
- LLMQ commitment tracking

## Database Schema (LMDB)

### Key-Value Design

LMDB is a key-value store. Schema design:

```
Database: tx_index
  Key:   TxID (32 bytes)
  Value: BlockHash (32 bytes) + TxIndex (4 bytes) + Height (4 bytes)

Database: block_index
  Key:   BlockHash (32 bytes)
  Value: Height (4 bytes) + Header (80 bytes) + TxCount (4 bytes)

Database: height_index
  Key:   Height (4 bytes, big-endian)
  Value: BlockHash (32 bytes)

Database: address_index (Phase 2)
  Key:   Address (variable) + TxID (32 bytes) + OutputIndex (4 bytes)
  Value: Value (8 bytes) + ScriptPubKey (variable)

Database: spent_index (Phase 2)
  Key:   TxID (32 bytes) + OutputIndex (4 bytes)
  Value: SpendingTxID (32 bytes) + SpendingInputIndex (4 bytes)

Database: masternode_payments (Phase 3)
  Key:   Height (4 bytes) + MasternodeProTxHash (32 bytes)
  Value: PaymentAmount (8 bytes)
```

## API Design

### gRPC Interface (Primary)

```protobuf
syntax = "proto3";

package dash.indexer.v1;

service IndexerService {
  // Get transaction by hash
  rpc GetTransaction(GetTransactionRequest) returns (GetTransactionResponse);

  // Get block by hash or height
  rpc GetBlock(GetBlockRequest) returns (GetBlockResponse);

  // Get address UTXOs (Phase 2)
  rpc GetAddressUtxos(GetAddressUtxosRequest) returns (GetAddressUtxosResponse);

  // Get transaction history for address (Phase 2)
  rpc GetAddressTransactions(GetAddressTransactionsRequest)
    returns (stream Transaction);

  // Get sync status
  rpc GetSyncStatus(GetSyncStatusRequest) returns (GetSyncStatusResponse);
}

message Transaction {
  bytes txid = 1;
  bytes raw_tx = 2;
  uint32 height = 3;
  bytes block_hash = 4;
  uint32 tx_index = 5;
  DashTxType tx_type = 6;
  bytes extra_payload = 7;  // For special transactions
}

enum DashTxType {
  NORMAL = 0;
  PROVIDER_REGISTER = 1;
  PROVIDER_UPDATE_SERVICE = 2;
  PROVIDER_UPDATE_REGISTRAR = 3;
  PROVIDER_UPDATE_REVOKE = 4;
  COINBASE = 5;
  QUORUM_COMMITMENT = 6;
  MNHF_SIGNAL = 7;
  ASSET_LOCK = 8;
  ASSET_UNLOCK = 9;
}
```

### JSON-RPC Interface (Compatibility)

Subset of dashd RPC for migration:
- `getrawtransaction`
- `getblock`
- `getblockhash`
- `getblockcount`

## Deployment

```bash
# Configuration file: dash-indexer.toml
[dashd]
zmq_endpoint = "tcp://127.0.0.1:28332"
rpc_url = "http://127.0.0.1:9998"
rpc_user = "user"
rpc_password = "pass"
datadir = "/path/to/.dashcore"  # For direct file reading

[indexer]
datadir = "/path/to/index-data"
bootstrap_from_files = true  # Use direct file reading for initial sync
indexes = ["tx", "address", "spent"]  # Which indexes to build

[api]
grpc_listen = "127.0.0.1:50051"
jsonrpc_listen = "127.0.0.1:3000"

[logging]
level = "info"
format = "json"  # or "pretty"
```

```bash
# Run the indexer
dash-indexer --config dash-indexer.toml

# Docker deployment (future)
docker run -v /data/dash:/dashd-data \
           -v /data/indexer:/indexer-data \
           -p 50051:50051 \
           dashpay/dash-indexer:latest
```

## Performance Considerations

### Bootstrap Performance

**Target**: Sync 2M blocks in < 24 hours on consumer hardware

**Optimizations**:
- Parallel block file processing (4-8 threads)
- Batch LMDB writes (1000s of transactions per commit)
- Zero-copy deserialization where possible
- Memory-mapped file reading

**Benchmark Goals** (Phase 2):
- Block processing: > 1000 blocks/sec
- Transaction indexing: > 10,000 tx/sec
- Query latency: < 10ms p99

### Real-time Performance

**Target**: Stay in sync with < 100ms lag

**ZMQ processing**:
- Async event loop
- Non-blocking writes
- Reorg handling

## Error Handling & Resilience

### Reorg Handling

```
1. Detect reorg (new block with lower height than tip)
2. Roll back indexes to fork point
3. Reindex from fork point forward
4. Resume normal operation
```

LMDB transactions make this atomic.

### Crash Recovery

- LMDB is crash-safe (ACID)
- Store last processed height in metadata DB
- On restart: resume from last height
- Can re-validate last N blocks for safety

## Testing Strategy

### Unit Tests
- Block parsing (Bitcoin + Dash special tx)
- Serialization/deserialization
- Database operations
- Reorg logic

### Integration Tests
- Full bootstrap from regtest
- ZMQ consumption
- API endpoints
- Reorg scenarios

### Performance Tests
- Bootstrap benchmark (regtest, testnet)
- Query performance
- Concurrent API requests

### Compatibility Tests
- Compare results with dashd indexes
- RPC response compatibility

## Migration Path

### Phase 1: Prototype (Weeks 1-2)
- [x] Design doc
- [ ] Basic Rust project structure
- [ ] ZMQ block consumer
- [ ] Simple tx index
- [ ] One gRPC endpoint

### Phase 2: Production-Ready (Months 1-3)
- [ ] All core indexes (tx, address, spent, timestamp)
- [ ] Full gRPC + JSON-RPC APIs
- [ ] Bootstrap from block files
- [ ] Reorg handling
- [ ] Production testing (mainnet sync)

### Phase 3: Dash Features (Months 3-6)
- [ ] Masternode payment index
- [ ] Governance index
- [ ] InstantSend index
- [ ] ChainLock metadata
- [ ] LLMQ commitment tracking

### Phase 4: Ecosystem Adoption (Months 6-12)
- [ ] Documentation for integration
- [ ] Migration tools for existing services
- [ ] Performance optimization
- [ ] Optional dashd integration (via configure flag)

### Phase 5: Future (Year 2+)
- [ ] GraphQL API
- [ ] Advanced analytics indexes
- [ ] Multi-instance deployment (read replicas)
- [ ] Potential: consensus engine modularization (like Zebra)

## Open Questions

1. **Mempool indexing**: Should we index unconfirmed transactions?
   - Zaino doesn't index mempool extensively
   - Could add lightweight mempool tracking for explorers

2. **Pruned node support**: Can we work with pruned dashd?
   - For bootstrap: need full blocks
   - For real-time: ZMQ works with pruned nodes
   - Could support: "indexer maintains full history even if dashd is pruned"

3. **Multiple indexes instances**: Should we support horizontal scaling?
   - Phase 1: Single instance
   - Future: Read replicas via LMDB replication

4. **Backward compatibility timeline**: How long to maintain JSON-RPC?
   - Recommend: 2-3 years, then deprecate
   - Focus on gRPC as primary

## Success Metrics

**Phase 1 (Prototype)**:
- ✅ ZMQ consumer works
- ✅ Can parse Dash special transactions
- ✅ Can query by txid via gRPC
- ✅ Proof of concept validated

**Phase 2 (Production)**:
- ⏱️ Mainnet sync in < 24 hours
- ⏱️ Query latency < 10ms p99
- ⏱️ Memory usage < 4GB
- ⏱️ 99.9% uptime

**Phase 3 (Adoption)**:
- 🎯 3+ block explorers using it
- 🎯 Wallet integration (mobile/desktop)
- 🎯 Community-contributed indexes
- 🎯 Reduced dashd load (node operators feedback)

## References

- [Zaino Repository](https://github.com/zingolabs/zaino)
- [Zebra Design](https://zebra.zfnd.org/dev/overview.html)
- [Dash Core Codebase](https://github.com/dashpay/dash)
- [LMDB Documentation](http://www.lmdb.tech/doc/)
- [Tonic gRPC Framework](https://github.com/hyperium/tonic)

## Appendix: Dash Block File Format

Dash uses Bitcoin's block file format (`blocks/blk*.dat`):

```
┌─────────────────────────────────────┐
│  Magic Bytes (4 bytes): 0xBF0C6BBD │  (Dash mainnet)
├─────────────────────────────────────┤
│  Block Size (4 bytes, little-endian)│
├─────────────────────────────────────┤
│  Block Header (80 bytes)            │
│    - nVersion (4 bytes)             │
│    - hashPrevBlock (32 bytes)       │
│    - hashMerkleRoot (32 bytes)      │
│    - nTime (4 bytes)                │
│    - nBits (4 bytes)                │
│    - nNonce (4 bytes)               │
├─────────────────────────────────────┤
│  Transaction Count (varint)         │
├─────────────────────────────────────┤
│  Transactions (variable)            │
│    - Each tx serialized             │
│    - Special tx have vExtraPayload  │
└─────────────────────────────────────┘
```

Magic bytes:
- Mainnet: `0xBF0C6BBD`
- Testnet: `0xFFCAE2CE`
- Regtest: `0xFCC1B7DC`
