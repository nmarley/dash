import plyvel
import struct

DB_PATH = "/app/data/blocks/index"


def read_compactsize(data, offset=0):
    """Read a CompactSize value from bytes (used for vectors/strings in network protocol)."""
    n = data[offset]
    offset += 1

    if n < 0xFD:
        return n, offset
    elif n == 0xFD:
        return struct.unpack("<H", data[offset : offset + 2])[0], offset + 2
    elif n == 0xFE:
        return struct.unpack("<I", data[offset : offset + 4])[0], offset + 4
    else:  # 0xff
        return struct.unpack("<Q", data[offset : offset + 8])[0], offset + 8


def read_varint(data, offset=0):
    """
    Read internal VarInt encoding from bytes.

    Uses 7 bits per byte for data, with MSB as continuation flag.
    This is the encoding used internally in LevelDB storage, not the network CompactSize.

    Algorithm from src/serialize.h:ReadVarInt():
    - Each byte has 7 bits of data (bits 0-6) and 1 continuation bit (bit 7)
    - If bit 7 is set (0x80), more bytes follow and we add 1 to accumulator
    - Bytes are in reverse order (first byte is most significant)
    """
    n = 0
    while True:
        if offset >= len(data):
            raise ValueError("VarInt extends beyond data")

        ch_data = data[offset]
        offset += 1

        # Shift accumulator left by 7 bits and OR with lower 7 bits of current byte
        n = (n << 7) | (ch_data & 0x7F)

        # If continuation bit is set, increment and continue
        if ch_data & 0x80:
            n += 1
        else:
            # No continuation bit, we're done
            return n, offset


def parse_block_locator(data):
    """Parse CBlockLocator from bytes."""
    offset = 0

    # Read version (int32)
    version = struct.unpack("<i", data[offset : offset + 4])[0]
    offset += 4

    # Read vector size (uses CompactSize for vector length)
    num_hashes, offset = read_compactsize(data, offset)

    # Read block hashes
    hashes = []
    for _ in range(num_hashes):
        hash_bytes = data[offset : offset + 32]
        hashes.append(hash_bytes[::-1].hex())  # reverse for display
        offset += 32

    return {"version": version, "num_hashes": num_hashes, "hashes": hashes}


def parse_disk_block_index(data):
    """
    Parse CDiskBlockIndex from bytes.

    From src/chain.h (CBlockIndex) and serialization in src/chain.cpp:
    - nVersion (int32)
    - nHeight (int32)
    - nStatus (uint32)
    - nTx (unsigned int) - number of transactions
    - nFile (int) - which blk*.dat file (if nStatus & BLOCK_HAVE_DATA)
    - nDataPos (unsigned int) - position in file (if nStatus & BLOCK_HAVE_DATA)
    - nUndoPos (unsigned int) - position in rev*.dat (if nStatus & BLOCK_HAVE_UNDO)
    - Block header (80 bytes):
      - nVersion (int32)
      - hashPrevBlock (uint256 - 32 bytes)
      - hashMerkleRoot (uint256 - 32 bytes)
      - nTime (uint32)
      - nBits (uint32)
      - nNonce (uint32)

    All integer fields use VarInt encoding in the index.
    """
    offset = 0

    # Read nVersion (VarInt)
    n_version, offset = read_varint(data, offset)

    # Read nHeight (VarInt)
    n_height, offset = read_varint(data, offset)

    # Read nStatus (VarInt)
    n_status, offset = read_varint(data, offset)

    # Read nTx (VarInt)
    n_tx, offset = read_varint(data, offset)

    # Constants from src/chain.h
    BLOCK_HAVE_DATA = 8  # full block available in blk*.dat
    BLOCK_HAVE_UNDO = 16  # undo data available in rev*.dat

    # Conditionally read file position data
    n_file = None
    n_data_pos = None
    n_undo_pos = None

    if n_status & BLOCK_HAVE_DATA:
        n_file, offset = read_varint(data, offset)
        n_data_pos, offset = read_varint(data, offset)

    if n_status & BLOCK_HAVE_UNDO:
        n_undo_pos, offset = read_varint(data, offset)

    # Read block hash (uint256 - 32 bytes)
    # This is the hash field in CDiskBlockIndex
    if offset + 32 > len(data):
        return {
            "version": n_version,
            "height": n_height,
            "status": n_status,
            "status_flags": {
                "have_data": bool(n_status & BLOCK_HAVE_DATA),
                "have_undo": bool(n_status & BLOCK_HAVE_UNDO),
            },
            "tx_count": n_tx,
            "file": n_file,
            "data_pos": n_data_pos,
            "undo_pos": n_undo_pos,
            "header": None,
        }

    block_hash = data[offset : offset + 32][::-1].hex()
    offset += 32

    # Read block header (CBlockHeader)
    # The header is stored with raw 80-byte format (NOT VarInt encoded)
    # Unlike network serialization, it's stored as-is from the block

    try:
        # Block header is 80 bytes exactly (standard Bitcoin/Dash header)
        if offset + 80 > len(data):
            raise ValueError(
                f"Not enough data for header: need 80 bytes, have {len(data) - offset}"
            )

        # Read version (4 bytes, little-endian int32)
        header_version = struct.unpack("<i", data[offset : offset + 4])[0]
        offset += 4

        # Read previous block hash (32 bytes, stored in internal byte order)
        hash_prev_block = data[offset : offset + 32][::-1].hex()
        offset += 32

        # Read merkle root hash (32 bytes, stored in internal byte order)
        hash_merkle_root = data[offset : offset + 32][::-1].hex()
        offset += 32

        # Read timestamp (4 bytes, little-endian uint32)
        n_time = struct.unpack("<I", data[offset : offset + 4])[0]
        offset += 4

        # Read bits (4 bytes, little-endian uint32)
        n_bits = struct.unpack("<I", data[offset : offset + 4])[0]
        offset += 4

        # Read nonce (4 bytes, little-endian uint32)
        n_nonce = struct.unpack("<I", data[offset : offset + 4])[0]
        offset += 4

    except (ValueError, IndexError, struct.error):
        # Not enough data for full header
        return {
            "version": n_version,
            "height": n_height,
            "status": n_status,
            "status_flags": {
                "have_data": bool(n_status & BLOCK_HAVE_DATA),
                "have_undo": bool(n_status & BLOCK_HAVE_UNDO),
            },
            "tx_count": n_tx,
            "file": n_file,
            "data_pos": n_data_pos,
            "undo_pos": n_undo_pos,
            "header": None,
        }

    return {
        "version": n_version,
        "height": n_height,
        "status": n_status,
        "status_flags": {
            "have_data": bool(n_status & BLOCK_HAVE_DATA),
            "have_undo": bool(n_status & BLOCK_HAVE_UNDO),
        },
        "tx_count": n_tx,
        "file": n_file,
        "data_pos": n_data_pos,
        "undo_pos": n_undo_pos,
        "header": {
            "version": header_version,
            "prev_block": hash_prev_block,
            "merkle_root": hash_merkle_root,
            "time": n_time,
            "bits": n_bits,
            "nonce": n_nonce,
        },
    }


def main():
    print("Parsing Dash block index database\n")
    db = plyvel.DB(DB_PATH, create_if_missing=False)

    for i, (k, v) in enumerate(db):
        key_type = k[0:1]

        if key_type == b"B":
            # Metadata entry - best block locator
            print(f"[{i}] METADATA (Best Block)")
            print(f"  Key: {k.hex()}")
            locator = parse_block_locator(v)
            print(f"  Version: {locator['version']}")
            print(f"  Chain contains {locator['num_hashes']} block hashes")
            print(
                f"  Tip hash: {locator['hashes'][0] if locator['hashes'] else 'none'}"
            )
            if len(locator["hashes"]) > 1:
                print(f"  ... and {len(locator['hashes']) - 1} more hashes")
            print()

        elif key_type == b"b":
            # Block index entry (key: 'b' + block_hash)
            block_hash = k[1:33][::-1].hex()  # reverse for display
            block_info = parse_disk_block_index(v)

            print(f"[{i}] BLOCK INDEX")
            print(f"  Block hash: {block_hash}")
            print(f"  Height: {block_info['height']}")
            print(f"  Version: {block_info['version']}")
            print(f"  Status: 0x{block_info['status']:08x}")
            print(f"    - Has block data: {block_info['status_flags']['have_data']}")
            print(f"    - Has undo data: {block_info['status_flags']['have_undo']}")
            print(f"  Transactions: {block_info['tx_count']}")

            if block_info["file"] is not None:
                print(f"  Block file: blk{block_info['file']:05d}.dat")
                print(f"  Data position: {block_info['data_pos']} bytes")

            if block_info["undo_pos"] is not None:
                print(f"  Undo position: {block_info['undo_pos']} bytes")

            if block_info["header"]:
                h = block_info["header"]
                print(f"  Header:")
                print(f"    Version: {h['version']}")
                print(f"    Previous: {h['prev_block']}")
                print(f"    Merkle root: {h['merkle_root']}")
                print(f"    Timestamp: {h['time']}")
                print(f"    Bits: 0x{h['bits']:08x}")
                print(f"    Nonce: {h['nonce']}")
            print()

        elif key_type == b"f":
            # File info entry (key: 'f' + file_number)
            # Value contains CBlockFileInfo with number of blocks, size, etc.
            file_num = struct.unpack("<I", k[1:5])[0]
            print(f"[{i}] FILE INFO")
            print(f"  File number: {file_num} (blk{file_num:05d}.dat)")
            print(f"  Key: {k.hex()}")
            print(f"  Value: {v.hex()}")
            # TODO: Parse CBlockFileInfo structure if needed
            print()

        elif key_type == b"l":
            # Last block file number used
            last_file = struct.unpack("<I", v)[0]
            print(f"[{i}] METADATA (Last Block File)")
            print(f"  Last file number: {last_file} (blk{last_file:05d}.dat)")
            print()

        elif key_type == b"R":
            # Reindex flag
            print(f"[{i}] METADATA (Reindex Flag)")
            print(f"  Value: {v.hex()}")
            print()

        elif key_type == b"F":
            # Flag for whether block file info is dirty
            print(f"[{i}] METADATA (Dirty Flag)")
            print(f"  Value: {v.hex()}")
            print()

        else:
            print(f"[{i}] UNKNOWN key type: {key_type.hex()}")
            print(f"  Key: {k.hex()}")
            print(f"  Value: {v.hex()}")
            print()

        if i >= 10:  # Limit output
            print("... (stopping after first 11 entries)")
            break

    db.close()


if __name__ == "__main__":
    main()
