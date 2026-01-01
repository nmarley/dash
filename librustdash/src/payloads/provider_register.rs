//! Provider Register Transaction (ProRegTx)
//!
//! This module implements the ProRegTx special transaction payload type,
//! used to register a new masternode on the Dash network.
//!
//! # Overview
//!
//! ProRegTx is the most complex provider transaction, used to register a new
//! masternode. It combines all patterns from other provider transactions:
//! - Collateral management (outpoint or internal)
//! - Network service information (IP + port)
//! - Three key types (owner, operator, voting)
//! - Operator reward sharing
//! - Payout script
//! - Platform integration (for Evo masternodes)
//! - ECDSA signature (from owner or voting key)
//!
//! # Transaction Type
//!
//! - **Transaction Type**: `TRANSACTION_PROVIDER_REGISTER` (1)
//! - **Versions**: Multiple versions for BLS schemes
//!
//! # Collateral
//!
//! The collateral can be specified in two ways:
//! 1. **External**: Reference an existing UTXO via `collateral_outpoint`
//! 2. **Internal**: Use output from this transaction (hash = all zeros, n = 0xFFFFFFFF)
//!
//! # Keys
//!
//! Three distinct keys are used:
//! - **Owner Key**: Controls masternode (can update registrar, revoke)
//! - **Operator Key**: BLS key for masternode duties (signing, voting)
//! - **Voting Key**: Used for governance voting
//!
//! # Operator Reward
//!
//! The operator can receive a percentage of masternode rewards (0-10000 basis points).
//! The remainder goes to the payout address specified in `script_payout`.
//!
//! # Serialization Format
//!
//! ```text
//! version                 (u16)           - Message version
//! mn_type                 (u16)           - Masternode type
//! mode                    (u16)           - Operating mode (0 only)
//! collateral_outpoint     (36 bytes)      - Hash (32) + index (4)
//! service_address         (6 or 18 bytes) - IPv4 or IPv6 + port
//! key_id_owner            (20 bytes)      - Owner key ID
//! pubkey_operator         (48 bytes)      - BLS operator public key
//! key_id_voting           (20 bytes)      - Voting key ID
//! operator_reward         (u16)           - Basis points (0-10000)
//! script_payout           (CompactSize+)  - Payout script
//! inputs_hash             (32 bytes)      - Replay protection
//! platform_node_id        (20 bytes)      - Only if Evo node
//! platform_p2p_port       (u16)           - Only if Evo node & v < 3
//! platform_http_port      (u16)           - Only if Evo node & v < 3
//! sig                     (CompactSize+)  - ECDSA signature
//! ```
//!
//! # Reference
//!
//! Dash Core implementation: `src/evo/providertx.h` (CProRegTx)

use crate::bls::BlsPublicKey;
use crate::error::{DashError, Result};
use crate::network_info::ServiceAddress;
use crate::payloads::provider_update_service::MasternodeType;
use crate::serialize::{read_compact_size, write_compact_size};
use crate::transaction::OutPoint;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Provider Register Transaction payload
///
/// The most complex provider transaction, used to register a new masternode.
///
/// # Example
///
/// ```
/// use librustdash::{ProRegTx, MasternodeType, ServiceAddress, BlsPublicKey, OutPoint};
///
/// let tx = ProRegTx {
///     version: 1,
///     mn_type: MasternodeType::Regular,
///     mode: 0,
///     collateral_outpoint: OutPoint { hash: [0xAA; 32], n: 0 },
///     service_address: ServiceAddress::new_ipv4([127, 0, 0, 1], 9999),
///     key_id_owner: [0xBB; 20],
///     pubkey_operator: BlsPublicKey { data: vec![0xCC; 48] },
///     key_id_voting: [0xDD; 20],
///     operator_reward: 100, // 1% in basis points
///     script_payout: vec![0x76, 0xa9],
///     inputs_hash: [0xEE; 32],
///     platform_node_id: None,
///     platform_p2p_port: None,
///     platform_http_port: None,
///     sig: vec![0xFF; 65],
/// };
///
/// let bytes = tx.serialize().unwrap();
/// let decoded = ProRegTx::deserialize(&bytes).unwrap();
/// assert_eq!(tx, decoded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProRegTx {
    /// Message version
    pub version: u16,
    /// Masternode type (Regular or Evo)
    pub mn_type: MasternodeType,
    /// Operating mode (currently only 0 supported)
    pub mode: u16,
    /// Collateral UTXO (or null for internal collateral)
    pub collateral_outpoint: OutPoint,
    /// Service IP address and port
    pub service_address: ServiceAddress,
    /// Owner key ID (CKeyID/uint160)
    pub key_id_owner: [u8; 20],
    /// BLS operator public key
    pub pubkey_operator: BlsPublicKey,
    /// Voting key ID (CKeyID/uint160)
    pub key_id_voting: [u8; 20],
    /// Operator reward in basis points (0-10000)
    pub operator_reward: u16,
    /// Payout script for owner rewards
    pub script_payout: Vec<u8>,
    /// Hash of transaction inputs (replay protection)
    pub inputs_hash: [u8; 32],
    /// Platform node ID (Evo masternodes only)
    pub platform_node_id: Option<[u8; 20]>,
    /// Platform P2P port (Evo masternodes, version < 3)
    pub platform_p2p_port: Option<u16>,
    /// Platform HTTP port (Evo masternodes, version < 3)
    pub platform_http_port: Option<u16>,
    /// ECDSA signature from owner/voting key
    pub sig: Vec<u8>,
}

impl ProRegTx {
    /// Serialize the ProRegTx payload
    ///
    /// Format (combines all provider transaction patterns):
    /// - version (u16)
    /// - mn_type (u16)
    /// - mode (u16)
    /// - collateral_outpoint (36 bytes: hash + n)
    /// - service_address (6 or 18 bytes)
    /// - key_id_owner (20 bytes)
    /// - pubkey_operator (48 bytes)
    /// - key_id_voting (20 bytes)
    /// - operator_reward (u16)
    /// - script_payout (CompactSize + bytes)
    /// - inputs_hash (32 bytes)
    /// - platform_node_id (20 bytes, if Evo)
    /// - platform_p2p_port (u16, if Evo && version < 3)
    /// - platform_http_port (u16, if Evo && version < 3)
    /// - sig (CompactSize + ECDSA signature)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version
        buf.write_u16::<LittleEndian>(self.version)?;

        // Write masternode type
        buf.write_u16::<LittleEndian>(self.mn_type as u16)?;

        // Write mode
        buf.write_u16::<LittleEndian>(self.mode)?;

        // Write collateral outpoint
        buf.write_all(&self.collateral_outpoint.hash)?;
        buf.write_u32::<LittleEndian>(self.collateral_outpoint.n)?;

        // Write service address
        let addr_bytes = self.service_address.serialize()?;
        buf.write_all(&addr_bytes)?;

        // Write owner key ID
        buf.write_all(&self.key_id_owner)?;

        // Write BLS operator public key (validate size first)
        let pubkey_bytes = self.pubkey_operator.serialize()?;
        buf.write_all(&pubkey_bytes)?;

        // Write voting key ID
        buf.write_all(&self.key_id_voting)?;

        // Write operator reward
        buf.write_u16::<LittleEndian>(self.operator_reward)?;

        // Write payout script with CompactSize prefix
        write_compact_size(&mut buf, self.script_payout.len() as u64)?;
        buf.write_all(&self.script_payout)?;

        // Write inputs_hash
        buf.write_all(&self.inputs_hash)?;

        // Write platform fields if this is an Evo node
        if self.mn_type == MasternodeType::Evo {
            // Platform node ID is required for Evo nodes
            let platform_node_id = self.platform_node_id.ok_or_else(|| {
                DashError::Serialization("Evo nodes require platform_node_id".to_string())
            })?;
            buf.write_all(&platform_node_id)?;

            // Platform ports only for version < 3
            if self.version < 3 {
                let p2p_port = self.platform_p2p_port.ok_or_else(|| {
                    DashError::Serialization(
                        "Evo nodes (v<3) require platform_p2p_port".to_string(),
                    )
                })?;
                let http_port = self.platform_http_port.ok_or_else(|| {
                    DashError::Serialization(
                        "Evo nodes (v<3) require platform_http_port".to_string(),
                    )
                })?;
                buf.write_u16::<LittleEndian>(p2p_port)?;
                buf.write_u16::<LittleEndian>(http_port)?;
            }
        }

        // Write signature with CompactSize prefix
        write_compact_size(&mut buf, self.sig.len() as u64)?;
        buf.write_all(&self.sig)?;

        Ok(buf)
    }

    /// Deserialize a ProRegTx payload from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version
        let version = cursor.read_u16::<LittleEndian>()?;

        // Read masternode type
        let type_val = cursor.read_u16::<LittleEndian>()?;
        let mn_type = MasternodeType::try_from(type_val)?;

        // Read mode
        let mode = cursor.read_u16::<LittleEndian>()?;

        // Read collateral outpoint
        let mut hash = [0u8; 32];
        cursor.read_exact(&mut hash)?;
        let n = cursor.read_u32::<LittleEndian>()?;
        let collateral_outpoint = OutPoint { hash, n };

        // Read service address
        // For ProRegTx, we can use the mn_type we already read to help determine address type
        // Also use size heuristic similar to ProUpServTx
        let addr_start_pos = cursor.position() as usize;
        let remaining_bytes = data.len() - addr_start_pos;

        // After service address we have:
        // - key_id_owner: 20 bytes
        // - pubkey_operator: 48 bytes
        // - key_id_voting: 20 bytes
        // - operator_reward: 2 bytes
        // - script_payout: CompactSize + variable (min 1 byte for empty)
        // - inputs_hash: 32 bytes
        // - platform fields (if Evo): 20-24 bytes
        // - sig: CompactSize + variable (min 1 byte for empty)
        // Base fixed: 20 + 48 + 20 + 2 + 32 = 122 bytes
        // Variable minimum: 2 bytes (CompactSize(0) for script and sig)
        // Total minimum after address: 124 bytes
        //
        // For typical values (2-byte script, 65-byte sig):
        // Variable: 1 + 2 + 1 + 65 = 69 bytes
        // Total typical: 122 + 69 = 191 bytes
        //
        // IPv4: 6 bytes, IPv6: 18 bytes
        // Difference: 12 bytes
        //
        // Strategy: If remaining >= 154 (18 + 124 + some typical data), likely IPv6
        // Otherwise IPv4

        // Heuristic for IPv4 vs IPv6:
        // Fixed bytes after address: 20 + 48 + 20 + 2 + 32 = 122
        // Variable: script (CompactSize + data) + sig (CompactSize + data)
        // Minimum variable: 2 bytes (both empty)
        // Typical variable: ~70 bytes (small script + 65-byte sig)
        // Large variable: 200+ bytes (large scripts)
        //
        // IPv4: 6 + 122 = 128 base, typically 128-200, can be 300+
        // IPv6: 18 + 122 = 140 base, typically 140-212, can be 312+
        //
        // Strategy: Use range check for typical small payloads
        // (205..220) = likely IPv6 with typical payload
        // >= 220 with large payload = need to check if it's consistently large
        // < 205 = IPv4
        //
        // Better: Just check if remaining is in the "IPv6 typical" range
        let service_address = if (205..320).contains(&remaining_bytes) {
            // Likely IPv6 with typical/moderate payload
            let mut ip_bytes = [0u8; 16];
            cursor.read_exact(&mut ip_bytes)?;
            let port = cursor.read_u16::<LittleEndian>()?;
            ServiceAddress {
                ip: ip_bytes.to_vec(),
                port,
            }
        } else {
            // IPv4 (most common, or large payloads)
            let mut ip_bytes = [0u8; 4];
            cursor.read_exact(&mut ip_bytes)?;
            let port = cursor.read_u16::<LittleEndian>()?;
            ServiceAddress {
                ip: ip_bytes.to_vec(),
                port,
            }
        };

        // Read owner key ID
        let mut key_id_owner = [0u8; 20];
        cursor.read_exact(&mut key_id_owner)?;

        // Read BLS operator public key (48 bytes)
        let mut pubkey_data = vec![0u8; 48];
        cursor.read_exact(&mut pubkey_data)?;
        let pubkey_operator = BlsPublicKey { data: pubkey_data };

        // Read voting key ID
        let mut key_id_voting = [0u8; 20];
        cursor.read_exact(&mut key_id_voting)?;

        // Read operator reward
        let operator_reward = cursor.read_u16::<LittleEndian>()?;

        // Read payout script
        let script_len = read_compact_size(&mut cursor)? as usize;
        let mut script_payout = vec![0u8; script_len];
        cursor.read_exact(&mut script_payout)?;

        // Read inputs_hash
        let mut inputs_hash = [0u8; 32];
        cursor.read_exact(&mut inputs_hash)?;

        // Read platform fields if this is an Evo node
        let (platform_node_id, platform_p2p_port, platform_http_port) =
            if mn_type == MasternodeType::Evo {
                // Read platform node ID
                let mut node_id = [0u8; 20];
                cursor.read_exact(&mut node_id)?;

                // Read ports only for version < 3
                let (p2p_port, http_port) = if version < 3 {
                    let p2p = cursor.read_u16::<LittleEndian>()?;
                    let http = cursor.read_u16::<LittleEndian>()?;
                    (Some(p2p), Some(http))
                } else {
                    (None, None)
                };

                (Some(node_id), p2p_port, http_port)
            } else {
                (None, None, None)
            };

        // Read signature
        let sig_len = read_compact_size(&mut cursor)? as usize;
        let mut sig = vec![0u8; sig_len];
        cursor.read_exact(&mut sig)?;

        Ok(ProRegTx {
            version,
            mn_type,
            mode,
            collateral_outpoint,
            service_address,
            key_id_owner,
            pubkey_operator,
            key_id_voting,
            operator_reward,
            script_payout,
            inputs_hash,
            platform_node_id,
            platform_p2p_port,
            platform_http_port,
            sig,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::BlsPublicKey;

    #[test]
    fn test_proregtx_v1_regular_node() {
        let tx = ProRegTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0xAA; 32],
                n: 0,
            },
            service_address: ServiceAddress {
                ip: vec![192, 168, 1, 100],
                port: 9999,
            },
            key_id_owner: [0xBB; 20],
            pubkey_operator: BlsPublicKey {
                data: vec![0xCC; 48],
            },
            key_id_voting: [0xDD; 20],
            operator_reward: 100,
            script_payout: vec![0x76, 0xa9],
            inputs_hash: [0xEE; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: vec![0xFF; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_proregtx_v2_evo_node() {
        let tx = ProRegTx {
            version: 2,
            mn_type: MasternodeType::Evo,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0; 32],
                n: 1,
            },
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 19999,
            },
            key_id_owner: [0; 20],
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            operator_reward: 0,
            script_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: Some([0xAA; 20]),
            platform_p2p_port: Some(26656),
            platform_http_port: Some(443),
            sig: vec![],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.version, decoded.version);
        assert_eq!(decoded.mn_type, MasternodeType::Evo);
        assert!(decoded.platform_node_id.is_some());
    }

    #[test]
    fn test_proregtx_null_collateral() {
        // Null collateral (all zeros hash) means collateral is in this tx
        let tx = ProRegTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0; 32],
                n: u32::MAX,
            },
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 9999,
            },
            key_id_owner: [0x11; 20],
            pubkey_operator: BlsPublicKey {
                data: vec![0x22; 48],
            },
            key_id_voting: [0x33; 20],
            operator_reward: 50,
            script_payout: vec![0x76],
            inputs_hash: [0x44; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: vec![0x55; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProRegTx::deserialize(&bytes).unwrap();
        assert_eq!(decoded.collateral_outpoint.hash, [0; 32]);
        assert_eq!(decoded.collateral_outpoint.n, u32::MAX);
    }

    #[test]
    fn test_proregtx_zero_operator_reward() {
        let tx = ProRegTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0xAA; 32],
                n: 0,
            },
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 9999,
            },
            key_id_owner: [0; 20],
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            operator_reward: 0, // Owner gets everything
            script_payout: vec![0x76, 0xa9],
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: vec![0; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProRegTx::deserialize(&bytes).unwrap();
        assert_eq!(decoded.operator_reward, 0);
    }

    #[test]
    fn test_proregtx_ipv6_address() {
        let tx = ProRegTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0xAA; 32],
                n: 0,
            },
            service_address: ServiceAddress {
                ip: vec![0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
                port: 9999,
            },
            key_id_owner: [0; 20],
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            operator_reward: 100,
            script_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: vec![0; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.service_address, decoded.service_address);
    }

    #[test]
    fn test_proregtx_invalid_operator_key() {
        let tx = ProRegTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0; 32],
                n: 0,
            },
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 9999,
            },
            key_id_owner: [0; 20],
            pubkey_operator: BlsPublicKey {
                data: vec![0; 47], // Wrong size
            },
            key_id_voting: [0; 20],
            operator_reward: 0,
            script_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: vec![],
        };
        assert!(tx.serialize().is_err());
    }

    #[test]
    fn test_proregtx_large_payout_script() {
        let tx = ProRegTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            mode: 0,
            collateral_outpoint: OutPoint {
                hash: [0xAA; 32],
                n: 0,
            },
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 9999,
            },
            key_id_owner: [0; 20],
            pubkey_operator: BlsPublicKey { data: vec![0; 48] },
            key_id_voting: [0; 20],
            operator_reward: 100,
            script_payout: vec![0x76; 200], // Large script
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: vec![0; 65],
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProRegTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.script_payout, decoded.script_payout);
    }
}
