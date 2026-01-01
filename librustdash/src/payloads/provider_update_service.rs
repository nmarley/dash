//! Provider Update Service Transaction (ProUpServTx)
//!
//! This module implements the ProUpServTx special transaction payload type,
//! used to update a masternode's service address and operator payout script.
//!
//! # Overview
//!
//! ProUpServTx allows a masternode operator to update:
//! - Service IP address and port
//! - Operator payout script
//! - Platform node information (for Evo masternodes only)
//!
//! Unlike ProUpRegTx, this transaction updates the service-related fields
//! and is signed with the operator's BLS key (not ECDSA from owner).
//!
//! # Transaction Type
//!
//! - **Transaction Type**: `TRANSACTION_PROVIDER_UPDATE_SERVICE` (2)
//! - **Versions**: 1 (Legacy BLS, no mn_type), 2 (Basic BLS, with mn_type)
//!
//! # Masternode Types
//!
//! - **Regular**: Traditional masternode (no platform fields)
//! - **Evo**: Evolution masternode with Dash Platform integration
//!
//! # Platform Fields
//!
//! For Evo masternodes (version >= 2):
//! - `platform_node_id`: 20-byte identifier for platform node
//! - `platform_p2p_port`: P2P port (only for version < 3)
//! - `platform_http_port`: HTTP API port (only for version < 3)
//!
//! # Serialization Format
//!
//! ```text
//! version                 (u16)           - Message version
//! mn_type                 (u16)           - Only if version >= 2
//! pro_tx_hash             (32 bytes)      - Hash of initial ProRegTx
//! service_address         (6 or 18 bytes) - IPv4 or IPv6 + port
//! script_operator_payout  (CompactSize+)  - Operator payout script
//! inputs_hash             (32 bytes)      - Replay protection
//! platform_node_id        (20 bytes)      - Only if Evo node
//! platform_p2p_port       (u16)           - Only if Evo node & v < 3
//! platform_http_port      (u16)           - Only if Evo node & v < 3
//! sig                     (96 bytes)      - BLS signature from operator
//! ```
//!
//! # Reference
//!
//! Dash Core implementation: `src/evo/providertx.h` (CProUpServTx)

use crate::bls::BlsSignature;
use crate::error::{DashError, Result};
use crate::network_info::ServiceAddress;
use crate::serialize::{read_compact_size, write_compact_size};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Masternode type
///
/// Indicates whether a masternode is a traditional masternode or
/// an Evolution (Evo) masternode with Dash Platform capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MasternodeType {
    /// Traditional masternode (no platform integration)
    Regular = 0,
    /// Evolution masternode with Dash Platform
    Evo = 1,
}

impl TryFrom<u16> for MasternodeType {
    type Error = DashError;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            0 => Ok(MasternodeType::Regular),
            1 => Ok(MasternodeType::Evo),
            _ => Err(DashError::Serialization(format!(
                "Invalid masternode type: {}",
                value
            ))),
        }
    }
}

/// Provider Update Service Transaction payload
///
/// Used to update a masternode's service address and operator payout script.
/// This is the most complex provider update transaction due to conditional
/// fields based on version and masternode type.
///
/// # Fields
///
/// - `version`: Message version (1 = Legacy BLS, 2 = Basic BLS)
/// - `mn_type`: Masternode type (only serialized if version >= 2)
/// - `pro_tx_hash`: Hash of the original ProRegTx
/// - `service_address`: IP address and port for masternode service
/// - `script_operator_payout`: Script for operator payments (variable length)
/// - `inputs_hash`: SHA256 hash of transaction inputs for replay protection
/// - `platform_node_id`: Platform node identifier (Evo nodes only)
/// - `platform_p2p_port`: Platform P2P port (Evo nodes with version < 3)
/// - `platform_http_port`: Platform HTTP port (Evo nodes with version < 3)
/// - `sig`: BLS signature from operator key
///
/// # Conditional Serialization
///
/// The serialization format varies based on `version` and `mn_type`:
///
/// - **Version 1**: No `mn_type` field, assumes Regular masternode
/// - **Version 2+**: Includes `mn_type` field
/// - **Evo nodes**: Include platform fields after `inputs_hash`
/// - **Version < 3**: Evo nodes include platform ports
/// - **Version >= 3**: Evo nodes omit platform ports (use ExtAddr)
///
/// # Example
///
/// ```
/// use librustdash::{ProUpServTx, MasternodeType, ServiceAddress, BlsSignature};
///
/// // Regular masternode (v2)
/// let regular_tx = ProUpServTx {
///     version: 2,
///     mn_type: MasternodeType::Regular,
///     pro_tx_hash: [0xAA; 32],
///     service_address: ServiceAddress::new_ipv4([127, 0, 0, 1], 9999),
///     script_operator_payout: vec![0x76, 0xa9],
///     inputs_hash: [0xBB; 32],
///     platform_node_id: None,
///     platform_p2p_port: None,
///     platform_http_port: None,
///     sig: BlsSignature { data: vec![0xCC; 96] },
/// };
///
/// // Evo masternode with platform (v2)
/// let evo_tx = ProUpServTx {
///     version: 2,
///     mn_type: MasternodeType::Evo,
///     pro_tx_hash: [0xAA; 32],
///     service_address: ServiceAddress::new_ipv4([127, 0, 0, 1], 19999),
///     script_operator_payout: vec![],
///     inputs_hash: [0xBB; 32],
///     platform_node_id: Some([0xDD; 20]),
///     platform_p2p_port: Some(26656),
///     platform_http_port: Some(443),
///     sig: BlsSignature { data: vec![0xCC; 96] },
/// };
///
/// let bytes = regular_tx.serialize().unwrap();
/// let decoded = ProUpServTx::deserialize(&bytes).unwrap();
/// assert_eq!(regular_tx, decoded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProUpServTx {
    /// Message version (1 = Legacy BLS, 2 = Basic BLS)
    pub version: u16,
    /// Masternode type (Regular or Evo)
    pub mn_type: MasternodeType,
    /// Hash of the original ProRegTx
    pub pro_tx_hash: [u8; 32],
    /// Service IP address and port
    pub service_address: ServiceAddress,
    /// Operator payout script (variable length)
    pub script_operator_payout: Vec<u8>,
    /// Hash of transaction inputs (replay protection)
    pub inputs_hash: [u8; 32],
    /// Platform node ID (Evo masternodes only)
    pub platform_node_id: Option<[u8; 20]>,
    /// Platform P2P port (Evo masternodes, version < 3)
    pub platform_p2p_port: Option<u16>,
    /// Platform HTTP API port (Evo masternodes, version < 3)
    pub platform_http_port: Option<u16>,
    /// BLS signature from operator key
    pub sig: BlsSignature,
}

impl ProUpServTx {
    /// Serialize the ProUpServTx payload
    ///
    /// Format varies by version and masternode type:
    /// - version (u16)
    /// - mn_type (u16, only if version >= 2)
    /// - pro_tx_hash (32 bytes)
    /// - service_address (6 or 18 bytes)
    /// - script_operator_payout (CompactSize + bytes)
    /// - inputs_hash (32 bytes)
    /// - platform_node_id (20 bytes, only if Evo node)
    /// - platform_p2p_port (u16, only if Evo node and version < 3)
    /// - platform_http_port (u16, only if Evo node and version < 3)
    /// - sig (96-byte BLS signature)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Write version
        buf.write_u16::<LittleEndian>(self.version)?;

        // Write masternode type (only for version >= 2)
        if self.version >= 2 {
            buf.write_u16::<LittleEndian>(self.mn_type as u16)?;
        }

        // Write pro_tx_hash
        buf.write_all(&self.pro_tx_hash)?;

        // Write service address
        let addr_bytes = self.service_address.serialize()?;
        buf.write_all(&addr_bytes)?;

        // Write operator payout script with CompactSize prefix
        write_compact_size(&mut buf, self.script_operator_payout.len() as u64)?;
        buf.write_all(&self.script_operator_payout)?;

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

        // Write BLS signature (validate size first)
        let sig_bytes = self.sig.serialize()?;
        buf.write_all(&sig_bytes)?;

        Ok(buf)
    }

    /// Deserialize a ProUpServTx payload from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read version
        let version = cursor.read_u16::<LittleEndian>()?;

        // Read masternode type (only for version >= 2)
        let mn_type = if version >= 2 {
            let type_val = cursor.read_u16::<LittleEndian>()?;
            MasternodeType::try_from(type_val)?
        } else {
            // Version 1 doesn't have mn_type field, defaults to Regular
            MasternodeType::Regular
        };

        // Read pro_tx_hash
        let mut pro_tx_hash = [0u8; 32];
        cursor.read_exact(&mut pro_tx_hash)?;

        // Read service address
        // Need to determine IPv4 (4 bytes + 2 port = 6 total) vs IPv6 (16 bytes + 2 port = 18 total)
        // Strategy: Use size heuristic based on remaining bytes
        let addr_start_pos = cursor.position() as usize;
        let remaining_bytes = data.len() - addr_start_pos;

        // Minimum bytes after address:
        // - script_operator_payout: min 1 byte (CompactSize(0))
        // - inputs_hash: 32 bytes
        // - sig: 96 bytes
        // Base minimum: 129 bytes
        //
        // For Evo nodes (version >= 2):
        // - platform_node_id: 20 bytes
        // - platform_p2p_port (v < 3): 2 bytes
        // - platform_http_port (v < 3): 2 bytes
        // Evo adds: 24 bytes (with ports) or 20 bytes (without)
        //
        // IPv4 + Regular: 6 + 129 = 135 bytes minimum
        // IPv4 + Evo (v2): 6 + 129 + 24 = 159 bytes minimum
        // IPv6 + Regular: 18 + 129 = 147 bytes minimum
        // IPv6 + Evo (v2): 18 + 129 + 24 = 171 bytes minimum
        //
        // Threshold: if remaining >= 147, could be IPv6
        // But need to distinguish from IPv4 + Evo (159)
        // Safe threshold: 147 <= remaining < 159 => IPv6 Regular
        //                 remaining >= 159 => could be IPv4 Evo or IPv6 Evo
        //                 remaining < 147 => IPv4 Regular

        let service_address = if (147..159).contains(&remaining_bytes) {
            // IPv6 Regular node
            let mut ip_bytes = [0u8; 16];
            cursor.read_exact(&mut ip_bytes)?;
            let port = cursor.read_u16::<LittleEndian>()?;
            ServiceAddress {
                ip: ip_bytes.to_vec(),
                port,
            }
        } else if remaining_bytes >= 171 {
            // IPv6 Evo node
            let mut ip_bytes = [0u8; 16];
            cursor.read_exact(&mut ip_bytes)?;
            let port = cursor.read_u16::<LittleEndian>()?;
            ServiceAddress {
                ip: ip_bytes.to_vec(),
                port,
            }
        } else {
            // IPv4 (most common)
            let mut ip_bytes = [0u8; 4];
            cursor.read_exact(&mut ip_bytes)?;
            let port = cursor.read_u16::<LittleEndian>()?;
            ServiceAddress {
                ip: ip_bytes.to_vec(),
                port,
            }
        };

        // Read operator payout script
        let script_len = read_compact_size(&mut cursor)? as usize;
        let mut script_operator_payout = vec![0u8; script_len];
        cursor.read_exact(&mut script_operator_payout)?;

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

        // Read BLS signature (96 bytes)
        let mut sig_data = vec![0u8; 96];
        cursor.read_exact(&mut sig_data)?;
        let sig = BlsSignature { data: sig_data };

        Ok(ProUpServTx {
            version,
            mn_type,
            pro_tx_hash,
            service_address,
            script_operator_payout,
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

    #[test]
    fn test_proupservtx_v1_roundtrip() {
        let tx = ProUpServTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            pro_tx_hash: [0xAA; 32],
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 9999,
            },
            script_operator_payout: vec![0x76, 0xa9],
            inputs_hash: [0xBB; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: BlsSignature {
                data: vec![0xCC; 96],
            },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpServTx::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_proupservtx_v2_regular_node() {
        let tx = ProUpServTx {
            version: 2,
            mn_type: MasternodeType::Regular,
            pro_tx_hash: [0; 32],
            service_address: ServiceAddress {
                ip: vec![0; 4],
                port: 19999,
            },
            script_operator_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: BlsSignature { data: vec![0; 96] },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpServTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.mn_type, MasternodeType::Regular);
        assert!(decoded.platform_node_id.is_none());
    }

    #[test]
    fn test_proupservtx_v2_evo_node_with_platform() {
        let tx = ProUpServTx {
            version: 2,
            mn_type: MasternodeType::Evo,
            pro_tx_hash: [0; 32],
            service_address: ServiceAddress {
                ip: vec![0; 4],
                port: 19999,
            },
            script_operator_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: Some([0xAA; 20]),
            platform_p2p_port: Some(26656),
            platform_http_port: Some(443),
            sig: BlsSignature { data: vec![0; 96] },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpServTx::deserialize(&bytes).unwrap();
        assert_eq!(decoded.mn_type, MasternodeType::Evo);
        assert!(decoded.platform_node_id.is_some());
        assert_eq!(decoded.platform_p2p_port, Some(26656));
        assert_eq!(decoded.platform_http_port, Some(443));
    }

    #[test]
    fn test_proupservtx_ipv6_address() {
        let tx = ProUpServTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            pro_tx_hash: [0; 32],
            service_address: ServiceAddress {
                ip: vec![0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
                port: 9999,
            },
            script_operator_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: BlsSignature { data: vec![0; 96] },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpServTx::deserialize(&bytes).unwrap();
        assert_eq!(tx.service_address, decoded.service_address);
    }

    #[test]
    fn test_proupservtx_empty_operator_payout() {
        let tx = ProUpServTx {
            version: 2,
            mn_type: MasternodeType::Regular,
            pro_tx_hash: [0x11; 32],
            service_address: ServiceAddress {
                ip: vec![192, 168, 1, 1],
                port: 9999,
            },
            script_operator_payout: vec![],
            inputs_hash: [0x22; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: BlsSignature {
                data: vec![0x33; 96],
            },
        };

        let bytes = tx.serialize().unwrap();
        let decoded = ProUpServTx::deserialize(&bytes).unwrap();
        assert_eq!(decoded.script_operator_payout.len(), 0);
    }

    #[test]
    fn test_proupservtx_invalid_signature() {
        let tx = ProUpServTx {
            version: 1,
            mn_type: MasternodeType::Regular,
            pro_tx_hash: [0; 32],
            service_address: ServiceAddress {
                ip: vec![127, 0, 0, 1],
                port: 9999,
            },
            script_operator_payout: vec![],
            inputs_hash: [0; 32],
            platform_node_id: None,
            platform_p2p_port: None,
            platform_http_port: None,
            sig: BlsSignature {
                data: vec![0; 48], // Wrong size
            },
        };
        assert!(tx.serialize().is_err());
    }

    #[test]
    fn test_masternode_type_values() {
        assert_eq!(MasternodeType::Regular as u16, 0);
        assert_eq!(MasternodeType::Evo as u16, 1);
    }
}
