//! Network address types for masternode service information
//!
//! Provides types for encoding masternode network addresses (IP + port).
//! Supports both IPv4 and IPv6 addresses.

use crate::error::{DashError, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Service address for a masternode
///
/// Contains IP address and port. IP can be either IPv4 (4 bytes) or IPv6 (16 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAddress {
    /// IP address bytes (4 bytes for IPv4, 16 bytes for IPv6)
    pub ip: Vec<u8>,
    /// Port number
    pub port: u16,
}

impl ServiceAddress {
    /// Create a new IPv4 service address
    pub fn new_ipv4(ip: [u8; 4], port: u16) -> Self {
        ServiceAddress {
            ip: ip.to_vec(),
            port,
        }
    }

    /// Create a new IPv6 service address
    pub fn new_ipv6(ip: [u8; 16], port: u16) -> Self {
        ServiceAddress {
            ip: ip.to_vec(),
            port,
        }
    }

    /// Check if this is an IPv4 address
    pub fn is_ipv4(&self) -> bool {
        self.ip.len() == 4
    }

    /// Check if this is an IPv6 address
    pub fn is_ipv6(&self) -> bool {
        self.ip.len() == 16
    }

    /// Serialize the service address
    ///
    /// Format: IP bytes + port (u16 little-endian)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        if self.ip.len() != 4 && self.ip.len() != 16 {
            return Err(DashError::Serialization(
                "IP address must be 4 bytes (IPv4) or 16 bytes (IPv6)".to_string(),
            ));
        }

        let mut buf = Vec::new();
        buf.write_all(&self.ip)?;
        buf.write_u16::<LittleEndian>(self.port)?;
        Ok(buf)
    }

    /// Deserialize a service address from bytes
    ///
    /// Expects either 6 bytes (IPv4 + port) or 18 bytes (IPv6 + port)
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        let ip = if data.len() == 6 {
            // IPv4: 4 bytes + 2 bytes port
            let mut ip_bytes = [0u8; 4];
            cursor.read_exact(&mut ip_bytes)?;
            ip_bytes.to_vec()
        } else if data.len() >= 18 {
            // IPv6: 16 bytes + 2 bytes port
            let mut ip_bytes = [0u8; 16];
            cursor.read_exact(&mut ip_bytes)?;
            ip_bytes.to_vec()
        } else {
            return Err(DashError::Serialization(
                "Invalid service address length".to_string(),
            ));
        };

        let port = cursor.read_u16::<LittleEndian>()?;

        Ok(ServiceAddress { ip, port })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_address_ipv4() {
        let addr = ServiceAddress {
            ip: vec![127, 0, 0, 1],
            port: 9999,
        };
        let bytes = addr.serialize().unwrap();

        // Should be 6 bytes: 4 for IP + 2 for port
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..4], &[127, 0, 0, 1]);

        let decoded = ServiceAddress::deserialize(&bytes).unwrap();
        assert_eq!(addr.ip, decoded.ip);
        assert_eq!(addr.port, decoded.port);
    }

    #[test]
    fn test_service_address_ipv6() {
        let addr = ServiceAddress {
            ip: vec![0; 16],
            port: 19999,
        };
        let bytes = addr.serialize().unwrap();

        // Should be 18 bytes: 16 for IP + 2 for port
        assert_eq!(bytes.len(), 18);

        let decoded = ServiceAddress::deserialize(&bytes).unwrap();
        assert_eq!(addr.ip, decoded.ip);
        assert_eq!(addr.port, decoded.port);
    }

    #[test]
    fn test_service_address_ipv4_roundtrip() {
        let addr = ServiceAddress {
            ip: vec![192, 168, 1, 100],
            port: 9999,
        };

        let bytes = addr.serialize().unwrap();
        let decoded = ServiceAddress::deserialize(&bytes).unwrap();

        assert_eq!(addr, decoded);
    }

    #[test]
    fn test_service_address_ipv6_roundtrip() {
        let addr = ServiceAddress {
            ip: vec![
                0x20, 0x01, 0x0d, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x01,
            ],
            port: 8080,
        };

        let bytes = addr.serialize().unwrap();
        let decoded = ServiceAddress::deserialize(&bytes).unwrap();

        assert_eq!(addr, decoded);
    }

    #[test]
    fn test_service_address_new_ipv4() {
        let addr = ServiceAddress::new_ipv4([127, 0, 0, 1], 9999);
        assert!(addr.is_ipv4());
        assert!(!addr.is_ipv6());
        assert_eq!(addr.ip, vec![127, 0, 0, 1]);
        assert_eq!(addr.port, 9999);
    }

    #[test]
    fn test_service_address_new_ipv6() {
        let addr = ServiceAddress::new_ipv6([0; 16], 19999);
        assert!(!addr.is_ipv4());
        assert!(addr.is_ipv6());
        assert_eq!(addr.ip.len(), 16);
        assert_eq!(addr.port, 19999);
    }

    #[test]
    fn test_service_address_invalid_ip_length() {
        let bad_addr = ServiceAddress {
            ip: vec![1, 2, 3], // Invalid length
            port: 9999,
        };
        assert!(bad_addr.serialize().is_err());
    }

    #[test]
    fn test_service_address_invalid_deserialize() {
        let bad_data = vec![1, 2, 3]; // Too short
        assert!(ServiceAddress::deserialize(&bad_data).is_err());
    }

    #[test]
    fn test_service_address_localhost() {
        let addr = ServiceAddress::new_ipv4([127, 0, 0, 1], 9999);
        let bytes = addr.serialize().unwrap();
        let decoded = ServiceAddress::deserialize(&bytes).unwrap();
        assert_eq!(addr, decoded);
    }

    #[test]
    fn test_service_address_typical_masternode() {
        // Typical mainnet masternode port
        let addr = ServiceAddress::new_ipv4([45, 76, 230, 123], 9999);
        let bytes = addr.serialize().unwrap();
        assert_eq!(bytes.len(), 6);

        let decoded = ServiceAddress::deserialize(&bytes).unwrap();
        assert_eq!(addr, decoded);
        assert_eq!(decoded.port, 9999);
    }

    #[test]
    fn test_service_address_testnet_port() {
        // Testnet uses port 19999
        let addr = ServiceAddress::new_ipv4([127, 0, 0, 1], 19999);
        assert_eq!(addr.port, 19999);

        let bytes = addr.serialize().unwrap();
        let decoded = ServiceAddress::deserialize(&bytes).unwrap();
        assert_eq!(decoded.port, 19999);
    }

    #[test]
    fn test_service_address_port_encoding() {
        let addr = ServiceAddress::new_ipv4([1, 2, 3, 4], 0x1234);
        let bytes = addr.serialize().unwrap();

        // Port should be little-endian
        // 0x1234 in little-endian is [0x34, 0x12]
        assert_eq!(bytes[4], 0x34);
        assert_eq!(bytes[5], 0x12);

        let decoded = ServiceAddress::deserialize(&bytes).unwrap();
        assert_eq!(decoded.port, 0x1234);
    }
}
