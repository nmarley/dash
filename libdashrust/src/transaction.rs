// Copyright (c) 2025 The Dash Core developers
// Distributed under the MIT software license

//! Transaction types and serialization

use crate::error::Result;
use crate::serialize::{read_compact_size, write_compact_size};
use crate::tx_type::DashTxType;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Transaction input
///
/// Corresponds to CTxIn in Dash Core
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxIn {
    /// Previous transaction output point
    pub previous_output: OutPoint,
    /// Signature script
    pub script_sig: Vec<u8>,
    /// Sequence number
    pub sequence: u32,
}

impl TxIn {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.previous_output.serialize(writer)?;
        write_compact_size(writer, self.script_sig.len() as u64)?;
        writer.write_all(&self.script_sig)?;
        writer.write_u32::<LittleEndian>(self.sequence)?;
        Ok(())
    }

    pub fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let previous_output = OutPoint::deserialize(reader)?;
        let script_len = read_compact_size(reader)? as usize;
        let mut script_sig = vec![0u8; script_len];
        reader.read_exact(&mut script_sig)?;
        let sequence = reader.read_u32::<LittleEndian>()?;

        Ok(TxIn {
            previous_output,
            script_sig,
            sequence,
        })
    }
}

/// Transaction output
///
/// Corresponds to CTxOut in Dash Core
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxOut {
    /// Value in satoshis
    pub value: i64,
    /// Public key script
    pub script_pubkey: Vec<u8>,
}

impl TxOut {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i64::<LittleEndian>(self.value)?;
        write_compact_size(writer, self.script_pubkey.len() as u64)?;
        writer.write_all(&self.script_pubkey)?;
        Ok(())
    }

    pub fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let value = reader.read_i64::<LittleEndian>()?;
        let script_len = read_compact_size(reader)? as usize;
        let mut script_pubkey = vec![0u8; script_len];
        reader.read_exact(&mut script_pubkey)?;

        Ok(TxOut {
            value,
            script_pubkey,
        })
    }
}

/// Outpoint - reference to a transaction output
///
/// Corresponds to COutPoint in Dash Core
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutPoint {
    /// Transaction hash
    pub hash: [u8; 32],
    /// Output index
    pub n: u32,
}

impl OutPoint {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.hash)?;
        writer.write_u32::<LittleEndian>(self.n)?;
        Ok(())
    }

    pub fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let mut hash = [0u8; 32];
        reader.read_exact(&mut hash)?;
        let n = reader.read_u32::<LittleEndian>()?;

        Ok(OutPoint { hash, n })
    }

    pub fn is_null(&self) -> bool {
        self.hash == [0u8; 32] && self.n == u32::MAX
    }
}

/// Dash transaction
///
/// Corresponds to CTransaction in Dash Core
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Transaction version (lower 16 bits of combined version/type field)
    pub version: i16,
    /// Transaction type (upper 16 bits of combined version/type field)
    pub tx_type: DashTxType,
    /// Transaction inputs
    pub inputs: Vec<TxIn>,
    /// Transaction outputs
    pub outputs: Vec<TxOut>,
    /// Lock time
    pub lock_time: u32,
    /// Extra payload (for special transactions)
    pub extra_payload: Option<Vec<u8>>,
}

impl Transaction {
    /// Serialize transaction to bytes
    ///
    /// Matches Dash Core's serialization format:
    /// - 4 bytes: combined version and type (type << 16 | version)
    /// - varint: input count
    /// - inputs
    /// - varint: output count  
    /// - outputs
    /// - 4 bytes: lock time
    /// - optional: extra payload (if version >= 3 && type != NORMAL)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Combine version and type into 32-bit value
        // n32bitVersion = (nType << 16) | nVersion
        let combined = ((self.tx_type as u32) << 16) | (self.version as u32 & 0xFFFF);
        buf.write_u32::<LittleEndian>(combined)?;

        // Serialize inputs
        write_compact_size(&mut buf, self.inputs.len() as u64)?;
        for input in &self.inputs {
            input.serialize(&mut buf)?;
        }

        // Serialize outputs
        write_compact_size(&mut buf, self.outputs.len() as u64)?;
        for output in &self.outputs {
            output.serialize(&mut buf)?;
        }

        // Lock time
        buf.write_u32::<LittleEndian>(self.lock_time)?;

        // Extra payload (if special tx)
        if self.version >= 3 && self.tx_type != DashTxType::Normal {
            if let Some(ref payload) = self.extra_payload {
                write_compact_size(&mut buf, payload.len() as u64)?;
                buf.write_all(payload)?;
            }
        }

        Ok(buf)
    }

    /// Deserialize transaction from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Read combined version/type
        let combined = cursor.read_u32::<LittleEndian>()?;
        let version = (combined & 0xFFFF) as i16;
        let tx_type = DashTxType::try_from(((combined >> 16) & 0xFFFF) as u16)?;

        // Read inputs
        let input_count = read_compact_size(&mut cursor)?;
        let mut inputs = Vec::with_capacity(input_count as usize);
        for _ in 0..input_count {
            inputs.push(TxIn::deserialize(&mut cursor)?);
        }

        // Read outputs
        let output_count = read_compact_size(&mut cursor)?;
        let mut outputs = Vec::with_capacity(output_count as usize);
        for _ in 0..output_count {
            outputs.push(TxOut::deserialize(&mut cursor)?);
        }

        // Lock time
        let lock_time = cursor.read_u32::<LittleEndian>()?;

        // Extra payload
        let extra_payload = if version >= 3 && tx_type != DashTxType::Normal {
            let payload_size = read_compact_size(&mut cursor)? as usize;
            let mut payload = vec![0u8; payload_size];
            cursor.read_exact(&mut payload)?;
            Some(payload)
        } else {
            None
        };

        Ok(Transaction {
            version,
            tx_type,
            inputs,
            outputs,
            lock_time,
            extra_payload,
        })
    }

    /// Check if this is a coinbase transaction
    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].previous_output.is_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_type_encoding() {
        // Normal tx: type=0, version=2
        let tx = Transaction {
            version: 2,
            tx_type: DashTxType::Normal,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
            extra_payload: None,
        };

        let bytes = tx.serialize().unwrap();
        // First 4 bytes should be 0x00000002
        assert_eq!(&bytes[0..4], &[0x02, 0x00, 0x00, 0x00]);

        // Special tx: type=1, version=3
        let tx = Transaction {
            version: 3,
            tx_type: DashTxType::ProviderRegister,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
            extra_payload: Some(vec![]),
        };

        let bytes = tx.serialize().unwrap();
        // First 4 bytes should be 0x00010003
        assert_eq!(&bytes[0..4], &[0x03, 0x00, 0x01, 0x00]);
    }

    #[test]
    fn test_transaction_roundtrip_empty() {
        let tx = Transaction {
            version: 2,
            tx_type: DashTxType::Normal,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
            extra_payload: None,
        };

        let bytes = tx.serialize().unwrap();
        let decoded = Transaction::deserialize(&bytes).unwrap();

        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_outpoint_null() {
        let outpoint = OutPoint {
            hash: [0u8; 32],
            n: u32::MAX,
        };
        assert!(outpoint.is_null());

        let outpoint = OutPoint {
            hash: [1u8; 32],
            n: 0,
        };
        assert!(!outpoint.is_null());
    }

    #[test]
    fn test_transaction_with_inputs_outputs() {
        let tx = Transaction {
            version: 2,
            tx_type: DashTxType::Normal,
            inputs: vec![TxIn {
                previous_output: OutPoint {
                    hash: [1u8; 32],
                    n: 0,
                },
                script_sig: vec![0x00, 0x01, 0x02],
                sequence: 0xFFFFFFFF,
            }],
            outputs: vec![
                TxOut {
                    value: 1000000,
                    script_pubkey: vec![0x76, 0xa9],
                },
                TxOut {
                    value: 500000,
                    script_pubkey: vec![0x88, 0xac],
                },
            ],
            lock_time: 0,
            extra_payload: None,
        };

        let bytes = tx.serialize().unwrap();
        let decoded = Transaction::deserialize(&bytes).unwrap();

        assert_eq!(tx, decoded);
        assert_eq!(decoded.inputs.len(), 1);
        assert_eq!(decoded.outputs.len(), 2);
        assert_eq!(decoded.outputs[0].value, 1000000);
    }

    #[test]
    fn test_coinbase_detection() {
        let tx = Transaction {
            version: 2,
            tx_type: DashTxType::Normal,
            inputs: vec![TxIn {
                previous_output: OutPoint {
                    hash: [0u8; 32],
                    n: u32::MAX,
                },
                script_sig: vec![],
                sequence: 0xFFFFFFFF,
            }],
            outputs: vec![],
            lock_time: 0,
            extra_payload: None,
        };

        assert!(tx.is_coinbase());
    }
}
