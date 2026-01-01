//! Quorum Commitment (LLMQ Core)
//!
//! This module implements quorum commitment structures for Dash's
//! Long-Living Masternode Quorums (LLMQ) system.
//!
//! # Overview
//!
//! LLMQs are core to Dash's advanced features:
//! - **InstantSend**: Transaction locking for instant confirmation
//! - **ChainLocks**: 51% attack prevention through quorum signing
//! - **Governance**: Decentralized masternode voting
//!
//! Quorum commitments represent the final result of a Distributed Key
//! Generation (DKG) session, where a subset of masternodes creates a
//! shared BLS threshold signature key.
//!
//! # LLMQ Types
//!
//! Different quorum sizes for different purposes:
//! - `Llmq50_60`: 50 members, 60% threshold (testing/development)
//! - `Llmq400_60`: 400 members, 60% threshold (InstantSend)
//! - `Llmq400_85`: 400 members, 85% threshold (ChainLocks)
//! - `Llmq100_67`: 100 members, 67% threshold (Platform)
//!
//! # Versions
//!
//! - v1: Legacy BLS, non-indexed (no rotation)
//! - v2: Legacy BLS, indexed (with quorum rotation)
//! - v3: Basic BLS, non-indexed
//! - v4: Basic BLS, indexed
//!
//! # Reference
//!
//! Dash Core implementation: `src/llmq/commitment.h`

use crate::bitvector::BitVector;
use crate::bls::{BlsPublicKey, BlsSignature};
use crate::error::{DashError, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// LLMQ Type identifiers
///
/// Different quorum configurations for different use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LLMQType {
    /// 50 members, 60% threshold
    Llmq50_60 = 1,
    /// 400 members, 60% threshold (InstantSend)
    Llmq400_60 = 2,
    /// 400 members, 85% threshold (ChainLocks)
    Llmq400_85 = 3,
    /// 100 members, 67% threshold (Platform)
    Llmq100_67 = 4,
    /// Test quorum
    LlmqTest = 100,
}

impl TryFrom<u8> for LLMQType {
    type Error = DashError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(LLMQType::Llmq50_60),
            2 => Ok(LLMQType::Llmq400_60),
            3 => Ok(LLMQType::Llmq400_85),
            4 => Ok(LLMQType::Llmq100_67),
            100 => Ok(LLMQType::LlmqTest),
            _ => Err(DashError::Serialization(format!(
                "Invalid LLMQ type: {}",
                value
            ))),
        }
    }
}

/// Final Commitment for an LLMQ quorum
///
/// Represents the final DKG (Distributed Key Generation) result for a quorum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalCommitment {
    pub version: u16,
    pub llmq_type: LLMQType,
    pub quorum_hash: [u8; 32],
    pub quorum_index: Option<i16>,
    pub signers: BitVector,
    pub valid_members: BitVector,
    pub quorum_public_key: BlsPublicKey,
    pub quorum_vvec_hash: [u8; 32],
    pub quorum_sig: BlsSignature,
    pub members_sig: BlsSignature,
}

impl FinalCommitment {
    /// Serialize the final commitment
    ///
    /// Format depends on version:
    /// - v1: Legacy BLS non-indexed (no quorum_index)
    /// - v2: Legacy BLS indexed (includes quorum_index)
    /// - v3: Basic BLS non-indexed
    /// - v4: Basic BLS indexed
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version
        buf.write_u16::<LittleEndian>(self.version)?;

        // Write LLMQ type
        buf.write_u8(self.llmq_type as u8)?;

        // Write quorum hash
        buf.write_all(&self.quorum_hash)?;

        // Write quorum index if this is an indexed version (2 or 4)
        if self.version == 2 || self.version == 4 {
            let index = self.quorum_index.ok_or_else(|| {
                DashError::Serialization("Indexed quorum requires quorum_index".to_string())
            })?;
            buf.write_i16::<LittleEndian>(index)?;
        }

        // Write signers BitVector
        let signers_bytes = self.signers.serialize()?;
        buf.write_all(&signers_bytes)?;

        // Write valid_members BitVector
        let valid_members_bytes = self.valid_members.serialize()?;
        buf.write_all(&valid_members_bytes)?;

        // Write quorum public key
        let pubkey_bytes = self.quorum_public_key.serialize()?;
        buf.write_all(&pubkey_bytes)?;

        // Write quorum vvec hash
        buf.write_all(&self.quorum_vvec_hash)?;

        // Write quorum signature
        let quorum_sig_bytes = self.quorum_sig.serialize()?;
        buf.write_all(&quorum_sig_bytes)?;

        // Write members signature
        let members_sig_bytes = self.members_sig.serialize()?;
        buf.write_all(&members_sig_bytes)?;

        Ok(buf)
    }

    /// Deserialize a final commitment from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version
        let version = cursor.read_u16::<LittleEndian>()?;

        // Read LLMQ type
        let llmq_type_val = cursor.read_u8()?;
        let llmq_type = LLMQType::try_from(llmq_type_val)?;

        // Read quorum hash
        let mut quorum_hash = [0u8; 32];
        cursor.read_exact(&mut quorum_hash)?;

        // Read quorum index if this is an indexed version (2 or 4)
        let quorum_index = if version == 2 || version == 4 {
            Some(cursor.read_i16::<LittleEndian>()?)
        } else {
            None
        };

        // Read signers BitVector
        let position = cursor.position() as usize;
        let signers = BitVector::deserialize(&data[position..])?;
        cursor.set_position(position as u64 + signers.serialize()?.len() as u64);

        // Read valid_members BitVector
        let position = cursor.position() as usize;
        let valid_members = BitVector::deserialize(&data[position..])?;
        cursor.set_position(position as u64 + valid_members.serialize()?.len() as u64);

        // Read quorum public key (48 bytes)
        let mut pubkey_data = vec![0u8; 48];
        cursor.read_exact(&mut pubkey_data)?;
        let quorum_public_key = BlsPublicKey { data: pubkey_data };

        // Read quorum vvec hash
        let mut quorum_vvec_hash = [0u8; 32];
        cursor.read_exact(&mut quorum_vvec_hash)?;

        // Read quorum signature (96 bytes)
        let mut quorum_sig_data = vec![0u8; 96];
        cursor.read_exact(&mut quorum_sig_data)?;
        let quorum_sig = BlsSignature {
            data: quorum_sig_data,
        };

        // Read members signature (96 bytes)
        let mut members_sig_data = vec![0u8; 96];
        cursor.read_exact(&mut members_sig_data)?;
        let members_sig = BlsSignature {
            data: members_sig_data,
        };

        Ok(FinalCommitment {
            version,
            llmq_type,
            quorum_hash,
            quorum_index,
            signers,
            valid_members,
            quorum_public_key,
            quorum_vvec_hash,
            quorum_sig,
            members_sig,
        })
    }
}

/// Quorum Commitment Transaction Payload
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumCommitmentPayload {
    pub version: u16,
    pub height: u32,
    pub commitment: FinalCommitment,
}

impl QuorumCommitmentPayload {
    /// Serialize the payload
    ///
    /// Format:
    /// - version (u16)
    /// - height (u32)
    /// - commitment (FinalCommitment)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version
        buf.write_u16::<LittleEndian>(self.version)?;

        // Write height
        buf.write_u32::<LittleEndian>(self.height)?;

        // Write commitment
        let commitment_bytes = self.commitment.serialize()?;
        buf.write_all(&commitment_bytes)?;

        Ok(buf)
    }

    /// Deserialize a payload from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version
        let version = cursor.read_u16::<LittleEndian>()?;

        // Read height
        let height = cursor.read_u32::<LittleEndian>()?;

        // Read commitment
        let position = cursor.position() as usize;
        let commitment = FinalCommitment::deserialize(&data[position..])?;

        Ok(QuorumCommitmentPayload {
            version,
            height,
            commitment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_final_commitment_v1_non_indexed() {
        let commitment = FinalCommitment {
            version: 1, // LEGACY_BLS_NON_INDEXED
            llmq_type: LLMQType::Llmq50_60,
            quorum_hash: [0xAA; 32],
            quorum_index: None,
            signers: BitVector {
                bits: vec![true, false, true, true, false, true, false, true], // 8 bits (1 byte)
            },
            valid_members: BitVector {
                bits: vec![true, true, false, true, false, false, true, true], // 8 bits (1 byte)
            },
            quorum_public_key: BlsPublicKey {
                data: vec![0xBB; 48],
            },
            quorum_vvec_hash: [0xCC; 32],
            quorum_sig: BlsSignature {
                data: vec![0xDD; 96],
            },
            members_sig: BlsSignature {
                data: vec![0xEE; 96],
            },
        };

        let bytes = commitment.serialize().unwrap();
        let decoded = FinalCommitment::deserialize(&bytes).unwrap();
        assert_eq!(commitment, decoded);
    }

    #[test]
    fn test_final_commitment_v2_indexed() {
        let commitment = FinalCommitment {
            version: 2, // LEGACY_BLS_INDEXED
            llmq_type: LLMQType::Llmq400_60,
            quorum_hash: [0; 32],
            quorum_index: Some(5),
            signers: BitVector {
                bits: vec![true; 400],
            },
            valid_members: BitVector {
                bits: vec![true; 400],
            },
            quorum_public_key: BlsPublicKey { data: vec![0; 48] },
            quorum_vvec_hash: [0; 32],
            quorum_sig: BlsSignature { data: vec![0; 96] },
            members_sig: BlsSignature { data: vec![0; 96] },
        };

        let bytes = commitment.serialize().unwrap();
        let decoded = FinalCommitment::deserialize(&bytes).unwrap();
        assert_eq!(commitment.quorum_index, Some(5));
        assert_eq!(decoded.quorum_index, Some(5));
    }

    #[test]
    fn test_final_commitment_v3_basic_bls() {
        let commitment = FinalCommitment {
            version: 3, // BASIC_BLS_NON_INDEXED
            llmq_type: LLMQType::Llmq50_60,
            quorum_hash: [0x11; 32],
            quorum_index: None,
            signers: BitVector {
                bits: vec![true; 50],
            },
            valid_members: BitVector {
                bits: vec![true; 50],
            },
            quorum_public_key: BlsPublicKey {
                data: vec![0x22; 48],
            },
            quorum_vvec_hash: [0x33; 32],
            quorum_sig: BlsSignature {
                data: vec![0x44; 96],
            },
            members_sig: BlsSignature {
                data: vec![0x55; 96],
            },
        };

        let bytes = commitment.serialize().unwrap();
        let decoded = FinalCommitment::deserialize(&bytes).unwrap();
        assert_eq!(commitment.version, 3);
        assert_eq!(decoded.version, 3);
    }

    #[test]
    fn test_quorum_commitment_payload() {
        let payload = QuorumCommitmentPayload {
            version: 1,
            height: 100000,
            commitment: FinalCommitment {
                version: 1,
                llmq_type: LLMQType::Llmq50_60,
                quorum_hash: [0; 32],
                quorum_index: None,
                signers: BitVector {
                    bits: vec![true; 50],
                },
                valid_members: BitVector {
                    bits: vec![true; 50],
                },
                quorum_public_key: BlsPublicKey { data: vec![0; 48] },
                quorum_vvec_hash: [0; 32],
                quorum_sig: BlsSignature { data: vec![0; 96] },
                members_sig: BlsSignature { data: vec![0; 96] },
            },
        };

        let bytes = payload.serialize().unwrap();
        let decoded = QuorumCommitmentPayload::deserialize(&bytes).unwrap();
        assert_eq!(payload.height, 100000);
        assert_eq!(decoded.height, 100000);
    }

    #[test]
    fn test_llmq_type_values() {
        assert_eq!(LLMQType::Llmq50_60 as u8, 1);
        assert_eq!(LLMQType::Llmq400_60 as u8, 2);
        assert_eq!(LLMQType::Llmq400_85 as u8, 3);
        assert_eq!(LLMQType::Llmq100_67 as u8, 4);
        assert_eq!(LLMQType::LlmqTest as u8, 100);
    }

    #[test]
    fn test_empty_bitvectors() {
        let commitment = FinalCommitment {
            version: 1,
            llmq_type: LLMQType::Llmq50_60,
            quorum_hash: [0; 32],
            quorum_index: None,
            signers: BitVector { bits: vec![] },
            valid_members: BitVector { bits: vec![] },
            quorum_public_key: BlsPublicKey { data: vec![0; 48] },
            quorum_vvec_hash: [0; 32],
            quorum_sig: BlsSignature { data: vec![0; 96] },
            members_sig: BlsSignature { data: vec![0; 96] },
        };

        let bytes = commitment.serialize().unwrap();
        let decoded = FinalCommitment::deserialize(&bytes).unwrap();
        assert_eq!(commitment, decoded);
    }
}
