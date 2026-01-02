import plyvel
import struct

DB_PATH = "/app/data/indexes/txindex"


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


def parse_disk_tx_pos(data):
    """
    Parse CDiskTxPos from bytes (FlatFilePos + nTxOffset).

    From src/index/disktxpos.h and src/flatfile.h:
    - nFile (int) - serialized as VARINT_MODE(NONNEGATIVE_SIGNED)
    - nPos (unsigned int) - serialized as VARINT
    - nTxOffset (unsigned int) - serialized as VARINT

    All use internal VarInt encoding, not CompactSize.
    """
    offset = 0

    # Read nFile (VarInt with NONNEGATIVE_SIGNED mode)
    n_file, offset = read_varint(data, offset)

    # Read nPos (VarInt DEFAULT mode)
    n_pos, offset = read_varint(data, offset)

    # Read nTxOffset (VarInt DEFAULT mode)
    n_tx_offset, offset = read_varint(data, offset)

    return {"file": n_file, "pos": n_pos, "tx_offset": n_tx_offset}


def main():
    print("Parsing Dash txindex database\n")
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

        elif key_type == b"t":
            # Transaction index entry
            txid = k[1:33][::-1].hex()  # reverse for display
            pos = parse_disk_tx_pos(v)

            print(f"[{i}] TX INDEX")
            print(f"  TXID: {txid}")
            print(f"  Block file: blk{pos['file']:05d}.dat")
            print(f"  Block position: {pos['pos']} bytes")
            print(f"  TX offset in block: {pos['tx_offset']} bytes")
            print()
        else:
            print(f"[{i}] UNKNOWN key type: {key_type.hex()}")
            print(f"  Key: {k.hex()}")
            print(f"  Value: {v.hex()}")
            print()

        if i > 10:  # Limit output
            print("... (stopping after first 11 entries)")
            break

    db.close()


if __name__ == "__main__":
    main()
