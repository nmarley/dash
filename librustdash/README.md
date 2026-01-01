# librustdash

Rust primitives library for Dash blockchain data structures.

## Overview

librustdash provides types and serialization for Dash blocks, transactions, and special transaction payloads, with byte-for-byte compatibility with Dash Core.

## Features

- **Transaction Types**: Full support for Dash transaction serialization including version/type encoding
- **Block Support**: BlockHeader and Block types with 80-byte header serialization
- **Special Transactions**: CbTx (Coinbase), AssetLock, AssetUnlock payloads
- **Type Safety**: Rust enums for transaction types with compile-time guarantees
- **Test Coverage**: 26 tests including roundtrip and golden tests
- **Dash Core Compatible**: Byte-for-byte serialization match

## Usage

```rust
use librustdash::{Transaction, DashTxType, Block};

// Deserialize a transaction
let tx_bytes = hex::decode("0200000001...").unwrap();
let tx = Transaction::deserialize(&tx_bytes).unwrap();

assert_eq!(tx.version, 2);
assert_eq!(tx.tx_type, DashTxType::Normal);

// Round-trip serialization
let serialized = tx.serialize().unwrap();
assert_eq!(serialized, tx_bytes); // Byte-for-byte match!

// Parse a block
let block_bytes = /* ... */;
let block = Block::deserialize(&block_bytes).unwrap();
println!("Block has {} transactions", block.transactions.len());
```

## Transaction Types

Supported Dash transaction types:

- `Normal` (0) - Standard transactions
- `ProviderRegister` (1) - Masternode registration
- `ProviderUpdateService` (2) - Masternode service update
- `ProviderUpdateRegistrar` (3) - Masternode registrar update
- `ProviderUpdateRevoke` (4) - Masternode revocation
- `Coinbase` (5) - Coinbase with extra payload
- `QuorumCommitment` (6) - LLMQ quorum commitment
- `MnhfSignal` (7) - Masternode hard fork signal
- `AssetLock` (8) - Platform asset lock
- `AssetUnlock` (9) - Platform asset unlock

## Special Transaction Payloads

### CbTx (Coinbase)

```rust
use librustdash::CbTx;

let cbtx = CbTx {
    version: 3,
    height: 100000,
    merkle_root_mn_list: [0xAA; 32],
    merkle_root_quorums: Some([0xBB; 32]),
    best_cl_height_diff: Some(10),
    best_cl_signature: Some(vec![0xFF; 96]), // BLS signature
    credit_pool_balance: Some(1234567890),
};

let bytes = cbtx.serialize().unwrap();
```

### AssetLock / AssetUnlock

```rust
use librustdash::{AssetLockPayload, AssetUnlockPayload, TxOut};

// Asset lock
let lock = AssetLockPayload {
    version: 1,
    credit_outputs: vec![
        TxOut {
            value: 1000000,
            script_pubkey: vec![0x76, 0xa9],
        }
    ],
};

// Asset unlock
let unlock = AssetUnlockPayload {
    version: 1,
    index: 123,
    fee: 1000,
    requested_height: 999999,
    quorum_hash: [0xAA; 32],
    quorum_sig: vec![0xBB; 96], // BLS signature
};
```

## Development

```bash
# Run tests
cargo test

# Run with output
cargo test -- --nocapture

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy -- -D warnings

# Build docs
cargo doc --no-deps --open
```

## Testing

The library includes comprehensive tests:

- **Unit tests**: Each component tested in isolation
- **Roundtrip tests**: Serialize → deserialize → serialize verification
- **Golden tests**: Real transaction data from Dash Core
- **Property tests**: Random generation (future)

## Implementation Status

### Phase 1: Primitives COMPLETE

- [x] Error types
- [x] CompactSize serialization
- [x] DashTxType enum
- [x] Transaction, TxIn, TxOut, OutPoint
- [x] BlockHeader and Block
- [x] CbTx payload (v1, v2, v3)
- [x] AssetLock/AssetUnlock payloads

### Future Phases

- [ ] ProRegTx and masternode payloads
- [ ] LLMQ commitment payload
- [ ] Hash computation utilities
- [ ] Property-based tests
- [ ] Fuzzing
- [ ] Performance benchmarks

## References

- [Dash Core](https://github.com/dashpay/dash) - Reference implementation
- [IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) - Detailed development plan
- [LIBRUSTDASH_DESIGN.md](docs/LIBRUSTDASH_DESIGN.md) - Design document

## License

MIT License - see LICENSE file for details

## Contributing

Contributions welcome! Please ensure:

1. All tests pass (`cargo test`)
2. Code is formatted (`cargo fmt`)
3. No clippy warnings (`cargo clippy`)
4. New functionality includes tests

---

**Status**: Phase 1 Complete - 26 tests passing
