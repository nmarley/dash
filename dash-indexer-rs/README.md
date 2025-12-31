# Dash Indexer (Rust)

Modular indexer service for the Dash blockchain, inspired by Zcash's Zaino.

## Status

**Phase 1 Prototype** - Work in Progress

- [x] Design document
- [x] Rust workspace scaffolding
- [x] Basic LMDB database layer
- [x] Type definitions for Dash transactions
- [ ] ZMQ consumer implementation
- [ ] Block file reader
- [ ] gRPC server
- [ ] Transaction indexing

## Architecture

```
dash-indexer-rs/
├── dash-indexer/     # Main binary and gRPC server
├── dash-state/       # LMDB database and storage layer
├── dash-sync/        # Block syncing (ZMQ + file reading)
└── proto/            # gRPC protocol definitions
```

## Building

```bash
cargo build
cargo test
```

## Running

```bash
# Create config file
cat > dash-indexer.toml <<EOF
[dashd]
zmq_endpoint = "tcp://127.0.0.1:28332"
rpc_url = "http://127.0.0.1:9998"
rpc_user = "user"
rpc_password = "pass"

[indexer]
datadir = "./index-data"
bootstrap_from_files = true
indexes = ["tx"]

[api]
grpc_listen = "127.0.0.1:50051"

[logging]
level = "info"
format = "pretty"
EOF

# Run indexer
cargo run -- --config dash-indexer.toml
```

## Development

See [INDEXER_DESIGN.md](../INDEXER_DESIGN.md) for full design documentation.

### Next Steps

1. Implement ZMQ consumer
2. Add Dash transaction parsing
3. Implement gRPC endpoints
4. Test with dashd regtest

## License

MIT
