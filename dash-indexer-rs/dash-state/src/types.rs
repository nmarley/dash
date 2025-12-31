use bitcoin::hashes::Hash;
use bitcoin::{BlockHash, Txid};
use serde::{Deserialize, Serialize};

/// Dash transaction types (from primitives/transaction.h)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum DashTxType {
    Normal = 0,
    ProviderRegister = 1,
    ProviderUpdateService = 2,
    ProviderUpdateRegistrar = 3,
    ProviderUpdateRevoke = 4,
    Coinbase = 5,
    QuorumCommitment = 6,
    MnhfSignal = 7,
    AssetLock = 8,
    AssetUnlock = 9,
}

impl From<u16> for DashTxType {
    fn from(value: u16) -> Self {
        match value {
            0 => DashTxType::Normal,
            1 => DashTxType::ProviderRegister,
            2 => DashTxType::ProviderUpdateService,
            3 => DashTxType::ProviderUpdateRegistrar,
            4 => DashTxType::ProviderUpdateRevoke,
            5 => DashTxType::Coinbase,
            6 => DashTxType::QuorumCommitment,
            7 => DashTxType::MnhfSignal,
            8 => DashTxType::AssetLock,
            9 => DashTxType::AssetUnlock,
            _ => DashTxType::Normal, // Default to normal
        }
    }
}

/// Block information stored in the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub hash: BlockHash,
    pub height: u32,
    pub header: Vec<u8>, // 80-byte block header
    pub tx_count: u32,
}

impl BlockInfo {
    pub fn new(hash: BlockHash, height: u32, header: Vec<u8>, tx_count: u32) -> Self {
        Self {
            hash,
            height,
            header,
            tx_count,
        }
    }

    /// Serialize for LMDB storage
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize BlockInfo")
    }

    /// Deserialize from LMDB storage
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        bincode::deserialize(bytes).map_err(|e| crate::StateError::Serialization(e.to_string()))
    }
}

/// Transaction information stored in the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: Txid,
    pub block_hash: BlockHash,
    pub height: u32,
    pub tx_index: u32,
    pub tx_type: DashTxType,
    pub extra_payload: Option<Vec<u8>>,
}

impl TransactionInfo {
    pub fn new(
        txid: Txid,
        block_hash: BlockHash,
        height: u32,
        tx_index: u32,
        tx_type: DashTxType,
        extra_payload: Option<Vec<u8>>,
    ) -> Self {
        Self {
            txid,
            block_hash,
            height,
            tx_index,
            tx_type,
            extra_payload,
        }
    }

    /// Serialize for LMDB storage
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize TransactionInfo")
    }

    /// Deserialize from LMDB storage
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        bincode::deserialize(bytes).map_err(|e| crate::StateError::Serialization(e.to_string()))
    }
}
