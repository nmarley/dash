//! Undo data structures for Dash blockchain reorganization
//!
//! These structures represent the data stored in rev*.dat files,
//! which allow the node to undo blocks during chain reorganizations.

use anyhow::{Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;

/// Represents a coin (UTXO) that was spent
///
/// This matches Dash Core's Coin class (src/coins.h)
#[derive(Debug, Clone)]
pub struct Coin {
    /// The transaction output that was spent
    pub txout: TxOut,
    /// Was this output from a coinbase transaction?
    pub is_coinbase: bool,
    /// Height of the block containing this output
    pub height: u32,
}

/// Transaction output
#[derive(Debug, Clone)]
pub struct TxOut {
    /// Amount in satoshis
    pub value: i64,
    /// ScriptPubKey
    pub script_pubkey: Vec<u8>,
}

impl Coin {
    /// Deserialize a Coin from undo data
    ///
    /// Format (from Dash Core's TxInUndoFormatter in undo.h):
    /// - varint: nCode = (nHeight * 2) + fCoinBase
    /// - 1 byte: dummy (0) if nHeight > 0 (compatibility)
    /// - compressed CTxOut
    pub fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        // Read nCode
        let n_code = read_varint(reader).context("Failed to read Coin nCode")?;

        let is_coinbase = (n_code & 1) != 0;
        let height = (n_code >> 1) as u32;

        // Read dummy varint if height > 0 (compatibility with older versions)
        // This was originally the transaction version
        if height > 0 {
            let _version_dummy = read_varint(reader).context("Failed to read version dummy")?;
        }

        // Read compressed TxOut
        let txout = TxOut::deserialize_compressed(reader)?;

        Ok(Coin {
            txout,
            is_coinbase,
            height,
        })
    }
}

impl TxOut {
    /// Deserialize a compressed TxOut
    ///
    /// Format (from Dash Core's CTxOutCompressor in compressor.h):
    /// - compressed amount (varint)
    /// - compressed script
    fn deserialize_compressed<R: Read>(reader: &mut R) -> Result<Self> {
        // Decompress amount
        let value = decompress_amount(read_varint(reader).context("Failed to read amount")?);

        // Decompress script
        let script_pubkey = decompress_script(reader).context("Failed to decompress script")?;

        Ok(TxOut {
            value,
            script_pubkey,
        })
    }
}

/// Undo information for a single transaction
///
/// This matches Dash Core's CTxUndo class (src/undo.h)
#[derive(Debug, Clone)]
pub struct CTxUndo {
    /// Previous outputs (coins) that were spent by this transaction
    pub vprevout: Vec<Coin>,
}

impl CTxUndo {
    /// Deserialize a CTxUndo from undo data
    pub fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        // Read vector of Coins
        let count = read_varint(reader).context("Failed to read CTxUndo coin count")?;

        let mut vprevout = Vec::with_capacity(count.min(10000) as usize);
        for i in 0..count {
            vprevout.push(
                Coin::deserialize(reader)
                    .with_context(|| format!("Failed to deserialize coin {}", i))?,
            );
        }

        Ok(CTxUndo { vprevout })
    }
}

/// Undo information for an entire block
///
/// This matches Dash Core's CBlockUndo class (src/undo.h)
#[derive(Debug, Clone)]
pub struct CBlockUndo {
    /// Undo information for each transaction (excluding coinbase)
    pub vtxundo: Vec<CTxUndo>,
}

impl CBlockUndo {
    /// Deserialize a CBlockUndo from undo data
    pub fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        // Read vector of CTxUndo
        let count = read_varint(reader).context("Failed to read CBlockUndo tx count")?;

        let mut vtxundo = Vec::with_capacity(count.min(10000) as usize);
        for i in 0..count {
            vtxundo.push(
                CTxUndo::deserialize(reader)
                    .with_context(|| format!("Failed to deserialize tx undo {}", i))?,
            );
        }

        Ok(CBlockUndo { vtxundo })
    }
}

/// Read a variable-length integer using VARINT encoding
///
/// This is Bitcoin's VarInt format (7-bit encoding), NOT CompactSize!
/// From Dash Core's ReadVarInt in serialize.h:
/// - Each byte stores 7 bits of data
/// - MSB indicates if more bytes follow
/// - Value is shifted and accumulated
fn read_varint<R: Read>(reader: &mut R) -> Result<u64> {
    let mut n: u64 = 0;
    loop {
        let ch_data = reader.read_u8().context("Failed to read varint byte")?;

        if n > (u64::MAX >> 7) {
            anyhow::bail!("VarInt too large");
        }

        n = (n << 7) | ((ch_data & 0x7F) as u64);

        if (ch_data & 0x80) != 0 {
            if n == u64::MAX {
                anyhow::bail!("VarInt too large");
            }
            n += 1;
        } else {
            return Ok(n);
        }
    }
}

/// Read a CompactSize variable-length integer
///
/// This is different from VARINT! Used in some places.
/// Format from Bitcoin/Dash protocol:
/// - < 0xfd: 1 byte
/// - 0xfd: next 2 bytes (little-endian)
/// - 0xfe: next 4 bytes (little-endian)
/// - 0xff: next 8 bytes (little-endian)
#[allow(dead_code)]
fn read_compact_size<R: Read>(reader: &mut R) -> Result<u64> {
    let first_byte = reader
        .read_u8()
        .context("Failed to read compact size first byte")?;

    match first_byte {
        0..=0xfc => Ok(first_byte as u64),
        0xfd => Ok(reader.read_u16::<LittleEndian>()? as u64),
        0xfe => Ok(reader.read_u32::<LittleEndian>()? as u64),
        0xff => Ok(reader.read_u64::<LittleEndian>()?),
    }
}

/// Decompress an amount value
///
/// From Dash Core's AmountCompression (compressor.h)
fn decompress_amount(x: u64) -> i64 {
    if x == 0 {
        return 0;
    }

    let mut x = x - 1;
    let e = x % 10;
    x /= 10;

    let mut n = if e < 9 {
        let d = (x % 9) + 1;
        d
    } else {
        x + 1
    };

    for _ in 0..e {
        n *= 10;
    }

    n as i64
}

/// Decompress a script
///
/// From Dash Core's ScriptCompression (compressor.h)
/// nSpecialScripts = 6, so scripts with nSize >= 6 are raw scripts
fn decompress_script<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let n_size = read_varint(reader).context("Failed to read script size")?;

    const N_SPECIAL_SCRIPTS: u64 = 6;

    match n_size {
        // P2PKH: 0x00 + 20-byte pubkey hash
        0 => {
            let mut script = vec![0x76, 0xa9, 0x14]; // OP_DUP OP_HASH160 PUSH(20)
            let mut hash = vec![0u8; 20];
            reader.read_exact(&mut hash)?;
            script.extend_from_slice(&hash);
            script.push(0x88); // OP_EQUALVERIFY
            script.push(0xac); // OP_CHECKSIG
            Ok(script)
        }
        // P2SH: 0x01 + 20-byte script hash
        1 => {
            let mut script = vec![0xa9, 0x14]; // OP_HASH160 PUSH(20)
            let mut hash = vec![0u8; 20];
            reader.read_exact(&mut hash)?;
            script.extend_from_slice(&hash);
            script.push(0x87); // OP_EQUAL
            Ok(script)
        }
        // P2PK (compressed): 0x02/0x03 + 32-byte pubkey
        2 | 3 => {
            let mut script = vec![0x21]; // PUSH(33)
            script.push(n_size as u8); // 0x02 or 0x03
            let mut pubkey = vec![0u8; 32];
            reader.read_exact(&mut pubkey)?;
            script.extend_from_slice(&pubkey);
            script.push(0xac); // OP_CHECKSIG
            Ok(script)
        }
        // P2PK (compressed pubkey that needs decompression): 0x04/0x05 prefix + 32 bytes
        // In Dash Core, this decompresses to a 65-byte uncompressed pubkey
        // For now, we'll just store the compressed form since we don't have pubkey decompression
        4 | 5 => {
            // Read 32 bytes of compressed pubkey data
            let mut compressed_pubkey = vec![n_size as u8 - 2]; // 0x02 or 0x03
            let mut pubkey_data = vec![0u8; 32];
            reader.read_exact(&mut pubkey_data)?;
            compressed_pubkey.extend_from_slice(&pubkey_data);

            // For now, return as compressed P2PK (33 bytes)
            // TODO: Implement full pubkey decompression to 65 bytes
            let mut script = vec![0x21]; // PUSH(33)
            script.extend_from_slice(&compressed_pubkey);
            script.push(0xac); // OP_CHECKSIG
            Ok(script)
        }
        // Other scripts: nSize >= 6 means raw script data
        // Actual script size is (nSize - nSpecialScripts)
        _ if n_size >= N_SPECIAL_SCRIPTS => {
            let size = (n_size - N_SPECIAL_SCRIPTS) as usize;
            let mut script = vec![0u8; size];
            reader.read_exact(&mut script)?;
            Ok(script)
        }
        _ => {
            anyhow::bail!("Invalid script compression size: {}", n_size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_varint() {
        let data = vec![0xfc];
        assert_eq!(read_varint(&mut &data[..]).unwrap(), 252);

        let data = vec![0xfd, 0xfd, 0x00];
        assert_eq!(read_varint(&mut &data[..]).unwrap(), 253);

        let data = vec![0xfe, 0x00, 0x00, 0x01, 0x00];
        assert_eq!(read_varint(&mut &data[..]).unwrap(), 65536);
    }

    #[test]
    fn test_decompress_amount() {
        assert_eq!(decompress_amount(0), 0);
        assert_eq!(decompress_amount(1), 1);
        assert_eq!(decompress_amount(10), 10);
    }
}
