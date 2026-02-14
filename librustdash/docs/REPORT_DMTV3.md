# dmtv3 -- Project Status Report

**Date:** February 14, 2026
**Location:** `/Users/nathan/projects/dmtv3`
**Branch:** `mn-wallet` (8 commits ahead of `master`, clean)
**Repository:** `https://github.com/nmarley/dmtv3.git`

## Overview

DMT v3 (Dash Masternode Tool v3) is a Rust-based terminal (TUI) cryptocurrency
wallet and masternode operator console for Dash. It is a modern replacement for
the Python-based dash-masternode-tool by Bertrand256, targeting power users and
masternode operators.

Key capabilities:
1. **Dash-aware operator wallet** -- manages masternode collateral UTXOs
   (1000/4000 DASH) with automatic collateral detection and locking
2. **Hardware wallet host** -- Ledger (and planned Trezor) support, especially
   important since Ledger Live and Trezor Suite have dropped Dash
3. **Masternode lifecycle console** -- deterministic masternode (ProTx) support
   with safe, auditable workflows

Design priorities: correctness over convenience, transparency over abstraction,
operator safety over UX shortcuts, no telemetry/ads/cloud services.

## Current State

- **~9,900 lines of Rust** across 3 crates
- **~55-60% complete** overall
- **Working tree clean**
- **Last activity:** January 20, 2026

## Architecture

Cargo workspace with 3 crates:

### dmt-core (~4,600 LOC)

Headless wallet logic:
- `address.rs` -- Address conversion (mainnet <-> testnet)
- `config.rs` -- TOML configuration
- `db.rs` -- SQLite persistence (949 LOC)
- `error.rs` -- Error types
- `masternode/` -- MN state tracking, sync
- `network/` -- Trait-based RPC abstraction (real + mock)
- `tx/` -- Transaction builder, coin selection, collateral safety
- `wallet/` -- UTXO classification, balance computation

### dmt-hw (~3,900 LOC)

Hardware wallet integration:
- `ledger/` -- Ledger APDU protocol (2,490 LOC), signing, tx construction
- `signer.rs` -- HardwareSigner trait
- 15 working examples
- 57 unit tests + 11 integration tests

### dmt-tui (~1,400 LOC)

Terminal UI (ratatui + crossterm):
- `app.rs` -- Application state machine
- `ui/` -- Views: wallet, masternodes, help

## Dependencies on Local Projects

- **librustdash** (`path = "../../../dash/librustdash"`) -- Transaction, TxIn,
  TxOut, OutPoint, DashTxType, ProRegTx, Block, BlockHeader
- **rust-ledger** (`path = "../../../rust-ledger"`) -- Ledger device
  communication

Both are path dependencies, not published crates.

## Completion Status by Phase

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 0 | Foundation (workspace + librustdash) | Complete |
| Phase 1 | Core application logic | ~70% |
| -- RPC Client | Dash Core RPC abstraction | Done (trait + RPC + mock) |
| -- Wallet | UTXO classification, coin control | Done |
| -- Tx builder | Build, coin select, safety checks | Done |
| -- Masternode | ProTx parsing, state tracking | Done |
| -- Configuration | TOML config, network settings | Done |
| Phase 2 | TUI proof of concept | ~40% |
| -- Basic app | State machine, views, keybindings | Scaffolded |
| -- Dashboard | Read-only display | Basic UI exists |
| -- RPC in TUI | Live data | Wired but untested E2E |
| Phase 3 | Hardware wallet integration | ~90% |
| -- Ledger | Address, signing, transactions | Complete (68 tests) |
| -- Trezor | Planned | Not started |
| Phase 4 | Safety features | ~50% |
| -- Collateral locking | In tx builder | Done |
| -- Typed confirmations | TUI-level | Not started |
| Phase 5 | Production hardening | Not started |

## Key Dependencies

| Dependency | Purpose |
|-----------|---------|
| librustdash (path) | Dash blockchain primitives |
| ledger-lib/ledger-proto (path) | Ledger device communication |
| ratatui 0.29 | Terminal UI framework |
| crossterm 0.28 | Terminal backend |
| tokio 1.x | Async runtime |
| reqwest 0.11 (rustls) | HTTP client for RPC |
| rusqlite 0.31 | SQLite database |
| bitcoin 0.32 | Bitcoin-compatible types |
| serde/toml | Configuration |

## Relationship to Other Projects

- **Consumes librustdash** -- primary downstream application
- **No dependency on daino** -- completely independent
- **Uses local rust-ledger** -- Ledger hardware wallet communication library

## Git History

Development ran from January 12-20, 2026. Phases built in order: dmt-hw
(Ledger signing), then dmt-core (network, wallet, tx, masternode), then dmt-tui
(scaffold). Work paused on Jan 20 after TUI scaffolding.
