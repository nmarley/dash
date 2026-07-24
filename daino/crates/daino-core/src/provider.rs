//! Abstractions over raw chain data retrieval.
//!
//! Today these are backed by Daino's LMDB indexes. Tomorrow a Rust
//! validator state service can implement the same traits without
//! rewriting the API layer (see `docs/VISION_RUST_VALIDATOR.md`).

use anyhow::Result;

/// Abstraction over raw transaction byte retrieval.
///
/// Backed by the `tx_raw` LMDB database today; by a validator state
/// service in a future Zaino-style architecture.
pub trait TxProvider {
    /// Fetch raw serialized transaction bytes by txid (internal byte order).
    fn get_raw_tx(&self, txid: &[u8; 32]) -> Result<Option<Vec<u8>>>;

    /// Fetch raw bytes for many txids. Default loops `get_raw_tx`.
    fn get_raw_tx_batch(&self, txids: &[[u8; 32]]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut out = Vec::with_capacity(txids.len());
        for txid in txids {
            out.push(self.get_raw_tx(txid)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapProvider {
        map: HashMap<[u8; 32], Vec<u8>>,
    }

    impl TxProvider for MapProvider {
        fn get_raw_tx(&self, txid: &[u8; 32]) -> Result<Option<Vec<u8>>> {
            Ok(self.map.get(txid).cloned())
        }
    }

    #[test]
    fn test_get_raw_tx_batch_default() {
        let mut map = HashMap::new();
        map.insert([1u8; 32], vec![0xAA, 0xBB]);
        map.insert([2u8; 32], vec![0xCC]);
        let p = MapProvider { map };

        let batch = p
            .get_raw_tx_batch(&[[1u8; 32], [3u8; 32], [2u8; 32]])
            .unwrap();
        assert_eq!(batch.len(), 3);
        assert_eq!(batch[0].as_ref().unwrap(), &vec![0xAA, 0xBB]);
        assert!(batch[1].is_none());
        assert_eq!(batch[2].as_ref().unwrap(), &vec![0xCC]);
    }
}
