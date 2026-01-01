# Dash Indexer Architecture Design

**Date:** January 1, 2026  
**Status:** Design Phase  
**Goal:** Build modular, testable indexer with pluggable block sources and storage backends

---

## Design Principles

1. **Separation of Concerns** - Block source ≠ Parser ≠ Indexer ≠ Storage
2. **Interface Segregation** - Traits for each abstraction
3. **Dependency Injection** - Components don't know about concrete implementations
4. **Testability** - Mock sources/storage for tests
5. **Composability** - Mix and match sources and backends

---

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Applications                      │
│  (gRPC API, REST API, CLI tools, block explorer frontend)   │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │
┌─────────────────────────────────────────────────────────────┐
│                      Indexer Core                            │
│  - Block processor                                           │
│  - Transaction indexer                                       │
│  - Address indexer                                           │
│  - Masternode tracker                                        │
│  - ChainLock tracker                                         │
└─────────────────────────────────────────────────────────────┘
                    ▲                     ▲
                    │                     │
        ┌───────────┴──────────┐   ┌─────┴──────────┐
        │   Block Source       │   │  Storage       │
        │   (Trait)            │   │  (Trait)       │
        └───────────┬──────────┘   └─────┬──────────┘
                    │                     │
        ┌───────────┴──────────┐   ┌─────┴──────────┐
        │ Implementations:     │   │ Implementations:│
        │ - File Reader        │   │ - LMDB         │
        │ - Network Sync       │   │ - RocksDB      │
        │ - ZMQ Stream         │   │ - SQLite       │
        │ - Memory (testing)   │   │ - Memory       │
        └──────────────────────┘   └────────────────┘
                    │
        ┌───────────┴──────────┐
        │   librustdash        │
        │   (Primitives)       │
        │ - Block/Tx parsing   │
        │ - Serialization      │
        │ - Hash computation   │
        └──────────────────────┘
```

---

## Core Traits

### 1. Block Source Trait

```rust
/// Abstraction over where blocks come from
pub trait BlockSource {
    /// Get the next block, if available
    fn next_block(&mut self) -> Result<Option<Block>>;
    
    /// Get current position/progress info
    fn position(&self) -> SourcePosition;
    
    /// Seek to a specific position (if supported)
    fn seek(&mut self, pos: SourcePosition) -> Result<()> {
        Err(Error::SeekNotSupported)
    }
    
    /// Check if this source supports seeking
    fn is_seekable(&self) -> bool {
        false
    }
}

/// Position in the block source
#[derive(Debug, Clone)]
pub enum SourcePosition {
    /// File-based source
    File {
        file_index: u32,      // blk00000.dat, blk00001.dat, etc.
        byte_offset: u64,     // Position within file
    },
    /// Network-based source
    Network {
        block_height: u32,    // Current chain height
        tip_hash: [u8; 32],   // Current chain tip
    },
    /// Stream-based source (ZMQ, websocket)
    Stream {
        sequence: u64,        // Message sequence number
    },
    /// Beginning of source
    Beginning,
    /// End of source
    End,
}
```

### 2. Storage Trait

```rust
/// Abstraction over block/transaction storage
pub trait BlockStore {
    /// Store a block (by height and/or hash)
    fn put_block(&mut self, height: u32, block: &Block) -> Result<()>;
    
    /// Retrieve a block by height
    fn get_block_by_height(&self, height: u32) -> Result<Option<Block>>;
    
    /// Retrieve a block by hash
    fn get_block_by_hash(&self, hash: &[u8; 32]) -> Result<Option<Block>>;
    
    /// Get the current tip (highest indexed block)
    fn get_tip(&self) -> Result<Option<(u32, [u8; 32])>>;
}

/// Abstraction over transaction storage
pub trait TxStore {
    /// Store a transaction
    fn put_tx(&mut self, txid: &[u8; 32], tx: &Transaction, block_height: u32) -> Result<()>;
    
    /// Retrieve a transaction by txid
    fn get_tx(&self, txid: &[u8; 32]) -> Result<Option<Transaction>>;
    
    /// Get all transactions in a block
    fn get_txs_in_block(&self, block_height: u32) -> Result<Vec<Transaction>>;
}

/// Abstraction over address indexing
pub trait AddressIndex {
    /// Index a transaction output by address
    fn index_output(&mut self, address: &str, txid: &[u8; 32], vout: u32, amount: i64, height: u32) -> Result<()>;
    
    /// Index a transaction input by address (spending)
    fn index_input(&mut self, address: &str, txid: &[u8; 32], vin: u32, height: u32) -> Result<()>;
    
    /// Get all UTXOs for an address
    fn get_utxos(&self, address: &str) -> Result<Vec<UTXO>>;
    
    /// Get transaction history for an address
    fn get_tx_history(&self, address: &str, limit: usize, offset: usize) -> Result<Vec<TxRef>>;
}

/// Abstraction over masternode tracking
pub trait MasternodeIndex {
    /// Track a new masternode registration
    fn index_registration(&mut self, protx_hash: &[u8; 32], protx: &ProRegTx, height: u32) -> Result<()>;
    
    /// Track a masternode update
    fn index_update(&mut self, protx_hash: &[u8; 32], update_type: MnUpdateType, height: u32) -> Result<()>;
    
    /// Get masternode by protx hash
    fn get_masternode(&self, protx_hash: &[u8; 32]) -> Result<Option<MasternodeInfo>>;
    
    /// Get all active masternodes at a given height
    fn get_active_masternodes(&self, height: u32) -> Result<Vec<MasternodeInfo>>;
}

/// Combined storage backend (composition of all stores)
pub trait Storage: BlockStore + TxStore + AddressIndex + MasternodeIndex {
    /// Begin a transaction (for atomic writes)
    fn begin_transaction(&mut self) -> Result<()>;
    
    /// Commit a transaction
    fn commit(&mut self) -> Result<()>;
    
    /// Rollback a transaction
    fn rollback(&mut self) -> Result<()>;
}
```

### 3. Block Processor Trait

```rust
/// Processes blocks from a source into storage
pub trait BlockProcessor {
    /// Process a single block
    fn process_block(&mut self, block: Block) -> Result<ProcessedBlock>;
    
    /// Get processing statistics
    fn stats(&self) -> ProcessingStats;
}

/// Result of processing a block
pub struct ProcessedBlock {
    pub height: u32,
    pub hash: [u8; 32],
    pub tx_count: usize,
    pub special_tx_count: usize,
    pub indexed_addresses: usize,
    pub masternode_updates: usize,
}

/// Processing statistics
pub struct ProcessingStats {
    pub blocks_processed: u64,
    pub txs_processed: u64,
    pub start_time: std::time::Instant,
    pub blocks_per_second: f64,
}
```

---

## Concrete Implementations

### Block Sources

#### 1. File-Based Block Source

```rust
/// Reads blocks from Dash Core's blk*.dat files
pub struct FileBlockSource {
    block_dir: PathBuf,
    current_file_index: u32,
    current_reader: Option<BlockFileReader>,
    magic_bytes: u32,
}

impl FileBlockSource {
    pub fn new(block_dir: PathBuf, network: Network) -> Result<Self> {
        let magic_bytes = match network {
            Network::Mainnet => 0xBF0C6BBD,
            Network::Testnet => 0xFFCAE2CE,
        };
        
        Ok(Self {
            block_dir,
            current_file_index: 0,
            current_reader: None,
            magic_bytes,
        })
    }
    
    fn open_next_file(&mut self) -> Result<bool> {
        let path = self.block_dir.join(format!("blk{:05}.dat", self.current_file_index));
        
        if !path.exists() {
            return Ok(false); // No more files
        }
        
        self.current_reader = Some(BlockFileReader::new(path, self.magic_bytes)?);
        self.current_file_index += 1;
        Ok(true)
    }
}

impl BlockSource for FileBlockSource {
    fn next_block(&mut self) -> Result<Option<Block>> {
        loop {
            // Try to read from current file
            if let Some(reader) = &mut self.current_reader {
                match reader.read_block()? {
                    Some(block) => return Ok(Some(block)),
                    None => {
                        // Current file exhausted, try next file
                        if !self.open_next_file()? {
                            return Ok(None); // No more files
                        }
                    }
                }
            } else {
                // Open first file
                if !self.open_next_file()? {
                    return Ok(None); // No files found
                }
            }
        }
    }
    
    fn position(&self) -> SourcePosition {
        if let Some(reader) = &self.current_reader {
            SourcePosition::File {
                file_index: self.current_file_index - 1,
                byte_offset: reader.position(),
            }
        } else {
            SourcePosition::Beginning
        }
    }
    
    fn is_seekable(&self) -> bool {
        true
    }
    
    fn seek(&mut self, pos: SourcePosition) -> Result<()> {
        match pos {
            SourcePosition::File { file_index, byte_offset } => {
                self.current_file_index = file_index;
                self.open_next_file()?;
                if let Some(reader) = &mut self.current_reader {
                    reader.seek(byte_offset)?;
                }
                Ok(())
            }
            _ => Err(Error::InvalidSeekPosition),
        }
    }
}

/// Low-level block file reader (NOT the public interface)
struct BlockFileReader {
    reader: BufReader<File>,
    magic_bytes: u32,
    position: u64,
}

impl BlockFileReader {
    fn new(path: PathBuf, magic_bytes: u32) -> Result<Self> {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        Ok(Self {
            reader,
            magic_bytes,
            position: 0,
        })
    }
    
    fn read_block(&mut self) -> Result<Option<Block>> {
        use byteorder::{LittleEndian, ReadBytesExt};
        
        // Read magic bytes
        let magic = match self.reader.read_u32::<LittleEndian>() {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        
        if magic != self.magic_bytes {
            return Err(Error::InvalidMagicBytes { expected: self.magic_bytes, found: magic });
        }
        
        self.position += 4;
        
        // Read size
        let size = self.reader.read_u32::<LittleEndian>()?;
        self.position += 4;
        
        // Read block data
        let mut block_data = vec![0u8; size as usize];
        self.reader.read_exact(&mut block_data)?;
        self.position += size as u64;
        
        // Parse using librustdash
        let block = Block::deserialize(&block_data)?;
        
        Ok(Some(block))
    }
    
    fn position(&self) -> u64 {
        self.position
    }
    
    fn seek(&mut self, pos: u64) -> Result<()> {
        use std::io::Seek;
        self.reader.seek(std::io::SeekFrom::Start(pos))?;
        self.position = pos;
        Ok(())
    }
}
```

#### 2. Network Block Source (Future)

```rust
/// Syncs blocks from network peers (P2P protocol)
pub struct NetworkBlockSource {
    peer_manager: PeerManager,
    current_height: u32,
    target_height: u32,
}

impl BlockSource for NetworkBlockSource {
    fn next_block(&mut self) -> Result<Option<Block>> {
        if self.current_height >= self.target_height {
            // Poll for new blocks
            self.peer_manager.update_chain_tip()?;
            self.target_height = self.peer_manager.best_height();
        }
        
        if self.current_height < self.target_height {
            let block = self.peer_manager.download_block(self.current_height)?;
            self.current_height += 1;
            Ok(Some(block))
        } else {
            Ok(None) // Caught up
        }
    }
    
    fn position(&self) -> SourcePosition {
        SourcePosition::Network {
            block_height: self.current_height,
            tip_hash: self.peer_manager.tip_hash(),
        }
    }
}
```

#### 3. ZMQ Stream Source (Future)

```rust
/// Receives real-time blocks from dashd ZMQ
pub struct ZmqBlockSource {
    zmq_socket: zmq::Socket,
    sequence: u64,
}

impl BlockSource for ZmqBlockSource {
    fn next_block(&mut self) -> Result<Option<Block>> {
        // Wait for next ZMQ message
        let msg = self.zmq_socket.recv_bytes(0)?;
        
        // Parse ZMQ message format
        let block = parse_zmq_block(&msg)?;
        self.sequence += 1;
        
        Ok(Some(block))
    }
    
    fn position(&self) -> SourcePosition {
        SourcePosition::Stream {
            sequence: self.sequence,
        }
    }
}
```

#### 4. Memory Source (Testing)

```rust
/// In-memory block source for testing
pub struct MemoryBlockSource {
    blocks: Vec<Block>,
    position: usize,
}

impl MemoryBlockSource {
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { blocks, position: 0 }
    }
}

impl BlockSource for MemoryBlockSource {
    fn next_block(&mut self) -> Result<Option<Block>> {
        if self.position < self.blocks.len() {
            let block = self.blocks[self.position].clone();
            self.position += 1;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
    
    fn position(&self) -> SourcePosition {
        SourcePosition::File {
            file_index: 0,
            byte_offset: self.position as u64,
        }
    }
}
```

---

## Storage Implementations

### 1. LMDB Storage

```rust
/// LMDB-based storage backend
pub struct LmdbStorage {
    env: lmdb::Environment,
    blocks_db: lmdb::Database,
    txs_db: lmdb::Database,
    address_db: lmdb::Database,
    masternode_db: lmdb::Database,
}

impl LmdbStorage {
    pub fn new(path: PathBuf) -> Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_dbs(10)
            .set_map_size(100 * 1024 * 1024 * 1024) // 100 GB
            .open(&path)?;
        
        let blocks_db = env.create_db(Some("blocks"), lmdb::DatabaseFlags::empty())?;
        let txs_db = env.create_db(Some("transactions"), lmdb::DatabaseFlags::empty())?;
        let address_db = env.create_db(Some("addresses"), lmdb::DatabaseFlags::DUP_SORT)?;
        let masternode_db = env.create_db(Some("masternodes"), lmdb::DatabaseFlags::empty())?;
        
        Ok(Self {
            env,
            blocks_db,
            txs_db,
            address_db,
            masternode_db,
        })
    }
}

impl BlockStore for LmdbStorage {
    fn put_block(&mut self, height: u32, block: &Block) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        
        // Serialize block
        let block_data = block.serialize()?;
        
        // Store by height
        let key = height.to_le_bytes();
        txn.put(self.blocks_db, &key, &block_data, lmdb::WriteFlags::empty())?;
        
        txn.commit()?;
        Ok(())
    }
    
    fn get_block_by_height(&self, height: u32) -> Result<Option<Block>> {
        let txn = self.env.begin_ro_txn()?;
        let key = height.to_le_bytes();
        
        match txn.get(self.blocks_db, &key) {
            Ok(data) => {
                let block = Block::deserialize(data)?;
                Ok(Some(block))
            }
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    fn get_block_by_hash(&self, hash: &[u8; 32]) -> Result<Option<Block>> {
        // Would need separate hash->height index
        todo!("Implement hash->height index")
    }
    
    fn get_tip(&self) -> Result<Option<(u32, [u8; 32])>> {
        // Read metadata
        todo!("Implement tip tracking")
    }
}

// Implement other traits (TxStore, AddressIndex, MasternodeIndex)...
```

### 2. RocksDB Storage (Alternative)

```rust
/// RocksDB-based storage backend
pub struct RocksDbStorage {
    db: rocksdb::DB,
}

impl RocksDbStorage {
    pub fn new(path: PathBuf) -> Result<Self> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.set_max_background_jobs(4);
        
        let db = rocksdb::DB::open(&opts, path)?;
        
        Ok(Self { db })
    }
}

impl BlockStore for RocksDbStorage {
    // Similar to LMDB implementation
    // RocksDB uses column families instead of separate databases
}
```

### 3. Memory Storage (Testing)

```rust
/// In-memory storage for testing
pub struct MemoryStorage {
    blocks: HashMap<u32, Block>,
    txs: HashMap<[u8; 32], Transaction>,
    addresses: HashMap<String, Vec<UTXO>>,
    masternodes: HashMap<[u8; 32], MasternodeInfo>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            txs: HashMap::new(),
            addresses: HashMap::new(),
            masternodes: HashMap::new(),
        }
    }
}

impl BlockStore for MemoryStorage {
    fn put_block(&mut self, height: u32, block: &Block) -> Result<()> {
        self.blocks.insert(height, block.clone());
        Ok(())
    }
    
    fn get_block_by_height(&self, height: u32) -> Result<Option<Block>> {
        Ok(self.blocks.get(&height).cloned())
    }
    
    // ... other methods
}
```

---

## Indexer Core

### Main Indexer

```rust
/// Main indexer that coordinates block processing
pub struct Indexer<S: BlockSource, T: Storage> {
    source: S,
    storage: T,
    processor: DefaultBlockProcessor,
    config: IndexerConfig,
}

impl<S: BlockSource, T: Storage> Indexer<S, T> {
    pub fn new(source: S, storage: T, config: IndexerConfig) -> Self {
        Self {
            source,
            storage,
            processor: DefaultBlockProcessor::new(),
            config,
        }
    }
    
    /// Run the indexer until source is exhausted
    pub fn run(&mut self) -> Result<()> {
        let mut stats = ProcessingStats::new();
        
        loop {
            // Get next block from source
            let block = match self.source.next_block()? {
                Some(b) => b,
                None => break, // Source exhausted
            };
            
            // Process block
            let processed = self.processor.process_block(block)?;
            
            // Store in database
            self.storage.begin_transaction()?;
            
            // Store block
            self.storage.put_block(processed.height, &processed.block)?;
            
            // Store transactions
            for (txid, tx) in processed.transactions {
                self.storage.put_tx(&txid, &tx, processed.height)?;
            }
            
            // Index addresses
            for (address, utxo) in processed.utxos {
                self.storage.index_output(&address, &utxo.txid, utxo.vout, utxo.amount, processed.height)?;
            }
            
            // Index masternodes
            for mn_update in processed.masternode_updates {
                self.storage.index_registration(&mn_update.protx_hash, &mn_update.protx, processed.height)?;
            }
            
            self.storage.commit()?;
            
            // Update stats
            stats.blocks_processed += 1;
            
            if stats.blocks_processed % 1000 == 0 {
                info!("Processed {} blocks, {:.2} blocks/sec", 
                      stats.blocks_processed, 
                      stats.blocks_per_second());
            }
        }
        
        info!("Indexing complete: {} blocks processed", stats.blocks_processed);
        Ok(())
    }
    
    /// Run until caught up, then wait for new blocks
    pub fn run_continuous(&mut self) -> Result<()> {
        loop {
            match self.source.next_block()? {
                Some(block) => {
                    // Process block...
                }
                None => {
                    // Wait for new blocks
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }
}
```

### Block Processor

```rust
/// Default block processor implementation
pub struct DefaultBlockProcessor {
    script_parser: ScriptParser,
}

impl DefaultBlockProcessor {
    pub fn new() -> Self {
        Self {
            script_parser: ScriptParser::new(),
        }
    }
    
    fn extract_addresses(&self, tx: &Transaction) -> Vec<(String, UTXO)> {
        let mut result = Vec::new();
        
        for (vout, output) in tx.outputs.iter().enumerate() {
            if let Some(address) = self.script_parser.extract_address(&output.script_pubkey) {
                result.push((
                    address,
                    UTXO {
                        txid: tx.txid().unwrap(),
                        vout: vout as u32,
                        amount: output.value,
                        script: output.script_pubkey.clone(),
                    }
                ));
            }
        }
        
        result
    }
    
    fn extract_masternode_updates(&self, tx: &Transaction) -> Vec<MasternodeUpdate> {
        use librustdash::DashTxType;
        
        match tx.tx_type {
            DashTxType::ProviderRegister => {
                if let Ok(protx) = ProRegTx::deserialize(&tx.extra_payload) {
                    vec![MasternodeUpdate::Registration {
                        protx_hash: tx.txid().unwrap(),
                        protx,
                    }]
                } else {
                    vec![]
                }
            }
            DashTxType::ProviderUpdateService => {
                // Handle service update...
                vec![]
            }
            // ... other types
            _ => vec![],
        }
    }
}

impl BlockProcessor for DefaultBlockProcessor {
    fn process_block(&mut self, block: Block) -> Result<ProcessedBlock> {
        let height = self.determine_height(&block)?; // Need to track this
        let hash = block.header.hash_prev_block; // For now, use prev hash
        
        let mut transactions = Vec::new();
        let mut utxos = Vec::new();
        let mut masternode_updates = Vec::new();
        let mut special_tx_count = 0;
        
        for tx in &block.transactions {
            let txid = tx.txid()?;
            
            // Store transaction
            transactions.push((txid, tx.clone()));
            
            // Extract addresses
            utxos.extend(self.extract_addresses(tx));
            
            // Extract special payloads
            if tx.tx_type != DashTxType::Classical {
                special_tx_count += 1;
                masternode_updates.extend(self.extract_masternode_updates(tx));
            }
        }
        
        Ok(ProcessedBlock {
            height,
            hash,
            block,
            transactions,
            utxos,
            masternode_updates,
            tx_count: transactions.len(),
            special_tx_count,
            indexed_addresses: utxos.len(),
        })
    }
    
    fn stats(&self) -> ProcessingStats {
        // Return stats...
        todo!()
    }
}
```

---

## Usage Examples

### Example 1: Index from Block Files

```rust
use dash_indexer::{Indexer, FileBlockSource, LmdbStorage, IndexerConfig};

fn main() -> Result<()> {
    // Setup block source (reads blk*.dat files)
    let source = FileBlockSource::new(
        PathBuf::from("/path/to/.dashcore/blocks"),
        Network::Mainnet
    )?;
    
    // Setup storage backend
    let storage = LmdbStorage::new(
        PathBuf::from("/path/to/index/data")
    )?;
    
    // Create indexer
    let mut indexer = Indexer::new(
        source,
        storage,
        IndexerConfig::default()
    );
    
    // Run indexer
    indexer.run()?;
    
    Ok(())
}
```

### Example 2: Index from Network (Future)

```rust
fn main() -> Result<()> {
    // Setup network source (P2P sync)
    let source = NetworkBlockSource::connect(vec![
        "seed1.dash.org:9999",
        "seed2.dash.org:9999",
    ])?;
    
    // Setup storage
    let storage = RocksDbStorage::new(
        PathBuf::from("/path/to/index/data")
    )?;
    
    // Create and run indexer
    let mut indexer = Indexer::new(source, storage, IndexerConfig::default());
    indexer.run_continuous()?; // Runs forever
    
    Ok(())
}
```

### Example 3: Testing with Mock Data

```rust
#[test]
fn test_indexer_with_mock_blocks() {
    // Create test blocks
    let blocks = vec![
        create_test_block(0),
        create_test_block(1),
        create_test_block(2),
    ];
    
    // Use memory source and storage
    let source = MemoryBlockSource::new(blocks);
    let storage = MemoryStorage::new();
    
    let mut indexer = Indexer::new(source, storage, IndexerConfig::default());
    indexer.run().unwrap();
    
    // Verify results
    assert_eq!(indexer.storage.blocks.len(), 3);
}
```

### Example 4: Hybrid Approach

```rust
fn main() -> Result<()> {
    // Start with file source for historical data
    let file_source = FileBlockSource::new(
        PathBuf::from("/path/to/.dashcore/blocks"),
        Network::Mainnet
    )?;
    
    let storage = LmdbStorage::new(PathBuf::from("/path/to/index/data"))?;
    
    let mut indexer = Indexer::new(file_source, storage, IndexerConfig::default());
    
    // Index historical blocks
    println!("Indexing historical blocks from files...");
    indexer.run()?;
    
    // Switch to ZMQ for real-time updates
    println!("Switching to real-time ZMQ stream...");
    let zmq_source = ZmqBlockSource::connect("tcp://localhost:28332")?;
    
    let mut live_indexer = Indexer::new(zmq_source, indexer.storage, IndexerConfig::default());
    live_indexer.run_continuous()?; // Run forever
    
    Ok(())
}
```

---

## Project Structure

```
dash-indexer/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Public API
│   ├── traits.rs                 # Core traits (BlockSource, Storage, etc.)
│   ├── indexer.rs                # Main Indexer struct
│   ├── processor.rs              # Block processor implementation
│   │
│   ├── sources/                  # Block source implementations
│   │   ├── mod.rs
│   │   ├── file.rs               # FileBlockSource
│   │   ├── network.rs            # NetworkBlockSource (future)
│   │   ├── zmq.rs                # ZmqBlockSource (future)
│   │   └── memory.rs             # MemoryBlockSource (testing)
│   │
│   ├── storage/                  # Storage implementations
│   │   ├── mod.rs
│   │   ├── lmdb.rs               # LmdbStorage
│   │   ├── rocksdb.rs            # RocksDbStorage
│   │   └── memory.rs             # MemoryStorage (testing)
│   │
│   ├── types.rs                  # Common types (UTXO, MasternodeInfo, etc.)
│   ├── script.rs                 # Script parsing (address extraction)
│   └── error.rs                  # Error types
│
├── tests/
│   ├── integration_tests.rs      # Integration tests
│   └── golden_tests.rs           # Tests with real Dash blocks
│
└── examples/
    ├── index_from_files.rs       # Example: index from blk*.dat
    ├── query_blocks.rs           # Example: query indexed data
    └── stats.rs                  # Example: indexer statistics
```

---

## Dependencies

```toml
[dependencies]
librustdash = { version = "0.1", path = "../librustdash" }
bitcoin = "0.32"                  # For address types, hashes

# Storage backends
lmdb = { version = "0.8", optional = true }
rocksdb = { version = "0.21", optional = true }

# Async runtime (for network source)
tokio = { version = "1.0", features = ["full"], optional = true }

# Logging
log = "0.4"
env_logger = "0.11"

# Serialization
byteorder = "1.5"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
tempfile = "3.0"                  # For test databases
proptest = "1.5"                  # Property-based testing

[features]
default = ["lmdb"]
lmdb-backend = ["lmdb"]
rocksdb-backend = ["rocksdb"]
network-sync = ["tokio"]
all = ["lmdb", "rocksdb", "tokio"]
```

---

## Implementation Phases

### Phase 1: Core Traits + File Source (Week 1)
- ✓ Define all traits
- ✓ Implement `FileBlockSource`
- ✓ Implement `MemoryStorage` (for testing)
- ✓ Basic `Indexer` that reads and stores blocks
- ✓ Integration tests

### Phase 2: LMDB Storage (Week 2)
- ✓ Implement `LmdbStorage`
- ✓ Block storage (by height)
- ✓ Transaction storage (by txid)
- ✓ Basic address indexing
- ✓ Migration from memory to LMDB

### Phase 3: Block Processor (Week 3)
- ✓ Script parsing (address extraction)
- ✓ UTXO tracking
- ✓ Special transaction processing
- ✓ Masternode indexing
- ✓ ChainLock tracking

### Phase 4: Query API (Week 4)
- ✓ gRPC server (or REST)
- ✓ Query blocks by height/hash
- ✓ Query transactions by txid
- ✓ Query address balance/history
- ✓ Query masternode list

### Future Phases
- Network block source (P2P sync)
- ZMQ real-time updates
- RocksDB backend
- Performance optimization
- Monitoring/metrics

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_file_source_reads_blocks() {
        let source = FileBlockSource::new(test_data_dir(), Network::Testnet).unwrap();
        // Test reading blocks...
    }
    
    #[test]
    fn test_memory_storage() {
        let mut storage = MemoryStorage::new();
        let block = create_test_block();
        storage.put_block(0, &block).unwrap();
        // Verify storage...
    }
}
```

### Integration Tests
```rust
#[test]
fn test_indexer_end_to_end() {
    // Create test blocks
    let blocks = vec![/* test blocks */];
    let source = MemoryBlockSource::new(blocks);
    let storage = MemoryStorage::new();
    
    let mut indexer = Indexer::new(source, storage, IndexerConfig::default());
    indexer.run().unwrap();
    
    // Query and verify
    let block = indexer.storage.get_block_by_height(0).unwrap();
    assert!(block.is_some());
}
```

### Golden Tests
```rust
#[test]
fn test_with_real_dash_blocks() {
    // Use actual Dash mainnet blocks from blk00000.dat
    let source = FileBlockSource::new(
        PathBuf::from("tests/data/mainnet"),
        Network::Mainnet
    ).unwrap();
    
    // Verify first 1000 blocks index correctly
    // ...
}
```

---

## Summary

**Key Design Decisions:**

1. ✅ **Trait-based abstractions** - Pluggable sources and storage
2. ✅ **Separation of concerns** - Source ≠ Parser ≠ Storage
3. ✅ **Testability** - Memory implementations for testing
4. ✅ **Composability** - Mix file + network sources
5. ✅ **No direct implementation on reader** - Reader is internal, traits are public

**Benefits:**

- Can switch from files → network → ZMQ without changing indexer
- Can switch from LMDB → RocksDB without changing logic
- Easy to test with mock data
- Easy to extend with new sources/backends

**Timeline:**

- Week 1: Traits + File source + Memory storage (working indexer)
- Week 2: LMDB storage (production-ready)
- Week 3: Block processor (full indexing)
- Week 4: Query API (usable indexer)

**Ready to implement?** You have the right architecture now!

---

**Author:** Architecture session with Nathan  
**Date:** January 1, 2026  
**Status:** Ready for implementation
