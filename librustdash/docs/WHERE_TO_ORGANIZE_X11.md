# Where Does X11 Hash Belong?

**Date:** January 2, 2026  
**Status:** Architectural Decision  
**Question:** Should X11 PoW hashing go in librustdash, or somewhere else?

---

## TL;DR

**X11 does NOT belong in librustdash primitives.**

X11 should be a **separate, optional component** - just like how librustzcash handles Equihash.

---

## The Answer from librustzcash

### How Zcash Handles Their PoW Algorithm (Equihash)

Looking at the librustzcash codebase structure:

```
librustzcash/
├── components/
│   └── equihash/          # ← PoW is a SEPARATE crate
│       ├── Cargo.toml
│       ├── build.rs       # Compiles C solver (optional)
│       ├── src/
│       │   ├── lib.rs     # Pure Rust verification
│       │   ├── verify.rs
│       │   └── blake2b.rs
│       └── tromp/         # C solver code (FFI, optional)
│           ├── equi_miner.c
│           └── equi.h
│
└── zcash_primitives/      # Main primitives crate
    ├── src/
    │   ├── block.rs       # pub use equihash; (re-export)
    │   ├── transaction.rs
    │   └── ...
    └── Cargo.toml         # equihash.workspace = true
```

### Key Design Choices in librustzcash

1. **Equihash is in `components/equihash`** - NOT embedded in `zcash_primitives`
2. **Pure Rust for verification** - Can validate PoW solutions
3. **FFI for solving** - Uses C code (`tromp/equi_miner.c`) behind optional feature flag
4. **zcash_primitives re-exports it** - `pub use equihash;` for convenience
5. **Optional solver feature** - `solver = ["dep:cc", "std"]`

### Their Cargo.toml

```toml
# components/equihash/Cargo.toml
[package]
name = "equihash"
description = "The Equihash Proof-of-Work function"

[features]
default = ["std"]
std = ["document-features"]

## Experimental tromp solver support, builds the C++ tromp solver and Rust FFI layer.
solver = ["dep:cc", "std"]

[dependencies]
blake2b_simd.workspace = true

[build-dependencies]
cc = { version = "1", optional = true }
```

### Their Build Script

```rust
// components/equihash/build.rs
#[cfg(feature = "solver")]
fn build_tromp_solver() {
    cc::Build::new()
        .include("tromp/")
        .file("tromp/equi_miner.c")
        .compile("equitromp");
}
```

**They use FFI to C code for the solver, but pure Rust for verification.**

---

## Recommended Architecture for Dash

Following the librustzcash pattern exactly:

```
librustdash/
├── components/
│   └── x11/                    # ← NEW separate crate
│       ├── Cargo.toml
│       ├── build.rs            # Compile C code via cc crate
│       ├── src/
│       │   ├── lib.rs          # Safe Rust wrapper API
│       │   └── ffi.rs          # Unsafe FFI bindings
│       ├── vendor/
│       │   └── sphlib/         # Bundled C code (Sphlib)
│       │       ├── sph_blake.c
│       │       ├── sph_bmw.c
│       │       ├── sph_groestl.c
│       │       └── ... (11 algorithms)
│       └── tests/
│           └── golden.rs       # 56+ Dash block test vectors
│
└── librustdash/                # Main primitives crate
    ├── src/
    │   ├── block.rs            # pub use x11; (optional re-export)
    │   ├── transaction.rs
    │   └── lib.rs
    └── Cargo.toml              # x11 = { workspace = true, optional = true }
```

---

## Why This Separation?

### 1. Separation of Concerns

**Primitives library:**
- Parse/serialize blocks and transactions
- Provide type-safe data structures
- Hash utilities (sha256d, txid, merkle root)
- NO validation logic

**PoW component:**
- X11 hash computation
- PoW verification (optional)
- Only needed for full validators, not indexers

### 2. Optional Dependency

**Who needs X11?**
- ✅ Full validators (dash-validator, future Zebra equivalent)
- ❌ Block indexers (daino) - trust dashd for validation
- ❌ Wallets - don't validate PoW
- ❌ Block explorers - don't validate PoW
- ❌ Analytics tools - don't validate PoW

**Most users of librustdash don't need X11 at all.**

### 3. Consensus Safety

X11 is **consensus-critical** code:
- Wrong hash = network fork
- Must be byte-for-byte compatible with Dash Core
- Battle-tested C implementation (10+ years in production)
- FFI approach reuses proven code, lower risk

### 4. Maintenance Burden

**Separate crate:**
- Independent versioning
- Can be updated without breaking librustdash
- Users who don't need it don't pay the cost

---

## Implementation Options

### Option 1: Workspace Component (Recommended)

Match librustzcash's pattern exactly:

```toml
# librustdash/Cargo.toml (workspace root)
[workspace]
members = [
    "librustdash",
    "components/x11",    # ← New component
]

[workspace.dependencies]
x11 = { version = "0.1", path = "components/x11" }

# components/x11/Cargo.toml
[package]
name = "x11"
description = "The X11 Proof-of-Work function for Dash"
version = "0.1.0"
license = "MIT"

[features]
default = []

[build-dependencies]
cc = "1"

# librustdash/Cargo.toml
[dependencies]
x11 = { workspace = true, optional = true }

[features]
pow = ["dep:x11"]  # Optional feature for PoW validation
```

**Usage:**
```rust
// In librustdash/src/block.rs
#[cfg(feature = "pow")]
pub use x11;

// Users who want PoW:
// librustdash = { version = "0.1", features = ["pow"] }

// Users who don't need PoW:
// librustdash = "0.1"
```

### Option 2: Completely Separate Crate (Even Better)

Don't even put it in librustdash workspace:

```
dash-x11/              # Separate repo or standalone crate
├── Cargo.toml
├── build.rs
├── vendor/sphlib/
├── src/
│   ├── lib.rs
│   └── ffi.rs
└── tests/

librustdash/          # No X11 at all
└── Cargo.toml        # No X11 dependency
```

**Usage:**
```toml
# Users who need PoW add it themselves:
[dependencies]
librustdash = "0.1"
dash-x11 = "0.1"  # Optional, separate crate
```

**Benefits:**
- Complete separation
- Zero coupling to librustdash
- Can be published independently
- Can evolve on different schedule

---

## Comparison: Zcash vs Dash

| Aspect | Zcash (Equihash) | Dash (X11) Recommendation |
|--------|------------------|---------------------------|
| **Location** | `components/equihash` | `components/x11` or separate repo |
| **Primitives dependency** | Optional re-export | Optional re-export or none at all |
| **Implementation** | Pure Rust verify + C FFI solver | C FFI for everything (battle-tested) |
| **Feature flag** | `solver` for C code | Similar approach |
| **Lines of code** | ~500 Rust + ~22k C | ~300 Rust wrapper + ~12k C |
| **Use case** | Verification needed | Verification rarely needed |
| **Consensus risk** | Medium | High (reuse C code) |

---

## Your Documentation Was Already Correct

Your existing analysis documents had this right:

### From `ARCHITECTURE.md` ✅

```
librustdash does NOT contain:
- ❌ POW verification
- ❌ Difficulty calculation
- ❌ Block validation rules

What librustdash DOES provide:
- ✓ BlockHeader::hash() - Compute the block hash (for indexing, lookups)
- ✓ BlockHeader::serialize() - Get 80-byte header for hashing

What dash_consensus WILL provide:
- ✓ x11_hash() - Compute X11 POW hash
- ✓ verify_pow() - Check if block meets target
- ✓ get_next_work_required() - Difficulty adjustment
```

### From `X11_RUST_PORT_ANALYSIS.md` ✅

**Question:** Is porting X11 to Rust worth it?

**Answer:** **NO, not for librustdash primitives.**

**Best Path Forward:**
1. Phase 1 (Current): Primitives library - NO X11 needed ✓
2. Phase 2 (Next): Block indexer - NO X11 needed ✓
3. Phase 3 (Future): Validator - Use FFI bindings (4-6 days)
4. Phase 4 (Maybe): Pure Rust port (2027+, if ever)

### From `X11_SUMMARY.md` ✅

**Verdict:** Wrong layer for X11. Keep it out of primitives. Use FFI later if needed.

---

## Implementation Timeline

### Phase 1: Now (2026 Q1-Q2) - **Do Nothing** ✅

Current focus:
- ✅ librustdash primitives (parsing, serialization)
- ✅ daino indexer development
- ❌ No X11 needed

### Phase 2: Later (2026 Q3+) - **Separate Component with FFI**

When building full validator (dash-validator):

1. Create `components/x11/` crate (or separate repo)
2. Bundle Sphlib C code in `vendor/`
3. Write safe Rust wrapper (~300 lines)
4. Use `cc` crate in build.rs to compile C code
5. Add golden tests with 56+ Dash block vectors
6. ~4-6 days effort total

### Phase 3: Never - **Pure Rust Port**

Don't do this:
- ❌ Not worth 4-6 weeks effort
- ❌ Consensus-critical = too risky
- ❌ No performance benefit for actual use cases
- ❌ Maintenance burden for 11 algorithm dependencies

---

## Code Example: Minimal x11 Crate

### components/x11/Cargo.toml

```toml
[package]
name = "x11"
version = "0.1.0"
description = "X11 Proof-of-Work hash function for Dash"
authors = ["Dash Community"]
license = "MIT"
edition = "2021"

[build-dependencies]
cc = "1"
```

### components/x11/build.rs

```rust
fn main() {
    // Compile all Sphlib algorithms
    let sources = [
        "vendor/sphlib/sph_blake.c",
        "vendor/sphlib/sph_bmw.c",
        "vendor/sphlib/sph_groestl.c",
        "vendor/sphlib/sph_jh.c",
        "vendor/sphlib/sph_keccak.c",
        "vendor/sphlib/sph_skein.c",
        "vendor/sphlib/sph_luffa.c",
        "vendor/sphlib/sph_cubehash.c",
        "vendor/sphlib/sph_shavite.c",
        "vendor/sphlib/sph_simd.c",
        "vendor/sphlib/sph_echo.c",
    ];

    cc::Build::new()
        .include("vendor/sphlib")
        .files(&sources)
        .compile("sphlib");

    println!("cargo:rerun-if-changed=vendor/sphlib");
}
```

### components/x11/src/lib.rs

```rust
//! X11 Proof-of-Work hash function for Dash.
//!
//! This crate provides X11 hashing via FFI to the battle-tested
//! Sphlib C implementation used in Dash Core.

use std::os::raw::c_uchar;

extern "C" {
    fn x11_hash(input: *const c_uchar, output: *mut c_uchar);
}

/// Compute X11 hash of 80-byte block header.
///
/// # Safety
///
/// Input must be exactly 80 bytes (block header).
/// Output will be exactly 32 bytes (hash).
pub fn hash_block_header(header: &[u8; 80]) -> [u8; 32] {
    let mut output = [0u8; 32];
    unsafe {
        x11_hash(header.as_ptr(), output.as_mut_ptr());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        // Dash mainnet genesis block header
        let header: [u8; 80] = [/* ... */];
        let hash = hash_block_header(&header);
        
        assert_eq!(
            hex::encode(hash),
            "00000ffd590b1485b3caadc19b22e6379c733355108f107a430458cdf3407ab6"
        );
    }
}
```

### components/x11/vendor/sphlib/x11.c

```c
#include "sph_blake.h"
#include "sph_bmw.h"
#include "sph_groestl.h"
#include "sph_jh.h"
#include "sph_keccak.h"
#include "sph_skein.h"
#include "sph_luffa.h"
#include "sph_cubehash.h"
#include "sph_shavite.h"
#include "sph_simd.h"
#include "sph_echo.h"

void x11_hash(const unsigned char* input, unsigned char* output)
{
    unsigned char hash[64];
    
    // Chain all 11 hash functions
    sph_blake512_context ctx_blake;
    sph_bmw512_context ctx_bmw;
    // ... (full implementation)
    
    // Blake
    sph_blake512_init(&ctx_blake);
    sph_blake512(&ctx_blake, input, 80);
    sph_blake512_close(&ctx_blake, hash);
    
    // BMW
    sph_bmw512_init(&ctx_bmw);
    sph_bmw512(&ctx_bmw, hash, 64);
    sph_bmw512_close(&ctx_bmw, hash);
    
    // ... continue for all 11 algorithms
    
    // Final output is first 32 bytes
    memcpy(output, hash, 32);
}
```

---

## Summary

### X11 belongs in:

- ❌ **NOT** in librustdash primitives (current crate)
- ❌ **NOT** in daino indexer
- ✅ **YES** in separate component crate (when needed)
- ✅ **YES** using FFI to C code (not pure Rust)
- ✅ **YES** following the librustzcash/equihash pattern exactly

### When to implement:

- **Now (Phase 1):** Do nothing - focus on primitives ✅
- **Later (Phase 2):** Add as separate component when building validator (~4-6 days)
- **Never:** Pure Rust port (not worth the effort)

### How to implement:

1. Create `components/x11/` workspace member
2. Bundle Sphlib C code
3. Use `cc` crate to compile C code
4. Safe Rust wrapper API
5. Optional feature in librustdash or completely separate

---

## References

- **librustzcash:** `/Users/nathan/projects/librustzcash`
  - `components/equihash/` - Their PoW component
  - `zcash_primitives/src/block.rs` - How they use it
- **Dash Core X11:** `/Users/nathan/projects/dash/src/crypto/x11`
- **Standalone X11:** `/Users/nathan/projects/x11-hash`
- **Related Docs:**
  - `ARCHITECTURE.md` - librustdash/dash_consensus separation
  - `X11_RUST_PORT_ANALYSIS.md` - Detailed effort analysis
  - `X11_SUMMARY.md` - Quick summary

---

**Author:** Architecture analysis session  
**Date:** January 2, 2026  
**Status:** Recommended approach - follows proven librustzcash pattern
