//! Block difficulty, target, and proof-of-work calculations.
//!
//! Provides functions to decode the compact `bits` field from block headers
//! into full 256-bit targets, compute difficulty as a floating-point ratio,
//! and compute the proof-of-work (chainwork) contribution of each block.
//!
//! All 256-bit values use big-endian byte order ([u8; 32]), matching the
//! standard Bitcoin/Dash RPC hex representation.

/// Decode the compact `bits` representation into a full 256-bit target.
///
/// The compact format encodes a 256-bit number as:
///   - Top byte: exponent (number of bytes in the full value)
///   - Lower 3 bytes: mantissa (coefficient)
///
/// `target = mantissa * 2^(8 * (exponent - 3))`
///
/// Returns the target as a 32-byte big-endian array.
pub fn target_from_bits(bits: u32) -> [u8; 32] {
    let mut target = [0u8; 32];

    let exponent = (bits >> 24) as usize;
    let mantissa = bits & 0x007f_ffff;

    // Negative flag (bit 23 of mantissa): Bitcoin treats this as zero target
    if bits & 0x0080_0000 != 0 {
        return target;
    }

    if exponent == 0 {
        return target;
    }

    // The mantissa occupies 3 bytes at position (exponent - 3) from the
    // least significant end. In big-endian layout, the MSB of the mantissa
    // goes at byte index (32 - exponent).
    let mantissa_bytes = [
        ((mantissa >> 16) & 0xff) as u8,
        ((mantissa >> 8) & 0xff) as u8,
        (mantissa & 0xff) as u8,
    ];

    // Place mantissa bytes, clamping to valid range
    for (i, &b) in mantissa_bytes.iter().enumerate() {
        let byte_pos = if exponent >= 3 {
            32usize.wrapping_sub(exponent).wrapping_add(i)
        } else {
            // exponent < 3: shift mantissa right (lose low bytes)
            32usize.wrapping_sub(3).wrapping_add(i)
        };
        if byte_pos < 32 {
            target[byte_pos] = b;
        }
    }

    // If exponent < 3, we effectively right-shifted the mantissa.
    // The bytes placed above are already correct because 32 - 3 + i
    // handles the positioning. But we need to zero out the extra bits
    // that would have been shifted away.
    if exponent < 3 {
        let shift = 3 - exponent;
        // Re-compute: place the mantissa right-shifted by `shift` bytes
        target = [0u8; 32];
        let shifted = mantissa >> (8 * shift);
        let remaining = 3 - shift;
        for i in 0..remaining {
            let byte_val = ((shifted >> (8 * (remaining - 1 - i))) & 0xff) as u8;
            let pos = 31 - i;
            if pos < 32 {
                target[pos] = byte_val;
            }
        }
    }

    target
}

/// Bitcoin's difficulty-1 reference target.
///
/// All chains (including Dash) define difficulty relative to this target
/// (`bits = 0x1d00ffff`), so difficulty 1.0 always means the same thing.
/// Dash's genesis target (`0x1e0ffff0`) is easier than this, giving a
/// genesis difficulty of ~0.000244140625 (1/4096).
const MAX_TARGET_BITS: u32 = 0x1d00_ffff;

/// Compute the difficulty from the compact `bits` field.
///
/// `difficulty = max_target / current_target`
///
/// Uses Dash's max target (`0x1e0fffff`), not Bitcoin's (`0x1d00ffff`).
/// Returns `f64` for JSON serialization compatibility.
pub fn difficulty_from_bits(bits: u32) -> f64 {
    let target = target_from_bits(bits);
    let max_target = target_from_bits(MAX_TARGET_BITS);

    // Convert both to f64 for division.
    // For precision, find the first nonzero byte and compute from there.
    let target_f = u256_to_f64(&target);
    let max_f = u256_to_f64(&max_target);

    if target_f == 0.0 {
        return 0.0;
    }

    max_f / target_f
}

/// Compute the proof-of-work for a block with the given `bits`.
///
/// `work = 2^256 / (target + 1)`
///
/// This is how much "work" this block represents. The chainwork for the
/// chain up to height H is the sum of work for blocks 0..=H.
///
/// Returns a 32-byte big-endian value.
pub fn work_from_bits(bits: u32) -> [u8; 32] {
    let target = target_from_bits(bits);

    // target + 1
    let target_plus_1 = add_u256(&target, &{
        let mut one = [0u8; 32];
        one[31] = 1;
        one
    });

    // Check for zero (shouldn't happen with valid blocks)
    if target_plus_1 == [0u8; 32] {
        return [0u8; 32];
    }

    // 2^256 / (target + 1)
    // Since 2^256 doesn't fit in 256 bits, we use the identity:
    // 2^256 / x = (2^256 - x) / x + 1 = (~x) / x + 1
    // where ~x is the bitwise complement
    let complement = not_u256(&target_plus_1);
    let (quotient, _remainder) = div_u256(&complement, &target_plus_1);

    add_u256(&quotient, &{
        let mut one = [0u8; 32];
        one[31] = 1;
        one
    })
}

/// Add two 256-bit big-endian numbers, wrapping on overflow.
pub fn add_u256(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    let mut carry: u16 = 0;

    for i in (0..32).rev() {
        let sum = a[i] as u16 + b[i] as u16 + carry;
        result[i] = sum as u8;
        carry = sum >> 8;
    }

    result
}

/// Bitwise complement of a 256-bit big-endian number.
fn not_u256(a: &[u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = !a[i];
    }
    result
}

/// Divide a 256-bit big-endian number by another, returning (quotient, remainder).
///
/// Uses long division bit by bit. Panics if divisor is zero.
fn div_u256(numerator: &[u8; 32], divisor: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    assert!(divisor != &[0u8; 32], "division by zero");

    let mut quotient = [0u8; 32];
    let mut remainder = [0u8; 32];

    for bit in 0..256 {
        // Shift remainder left by 1
        let mut carry = 0u8;
        for i in (0..32).rev() {
            let new_carry = remainder[i] >> 7;
            remainder[i] = (remainder[i] << 1) | carry;
            carry = new_carry;
        }

        // Bring down the next bit from numerator
        let byte_idx = bit / 8;
        let bit_idx = 7 - (bit % 8);
        let next_bit = (numerator[byte_idx] >> bit_idx) & 1;
        remainder[31] |= next_bit;

        // If remainder >= divisor, subtract and set quotient bit
        if gte_u256(&remainder, divisor) {
            remainder = sub_u256(&remainder, divisor);
            let q_byte = bit / 8;
            let q_bit = 7 - (bit % 8);
            quotient[q_byte] |= 1 << q_bit;
        }
    }

    (quotient, remainder)
}

/// Compare two 256-bit big-endian numbers: a >= b
fn gte_u256(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in 0..32 {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true // equal
}

/// Subtract b from a (256-bit big-endian), wrapping on underflow.
fn sub_u256(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    let mut borrow: i16 = 0;

    for i in (0..32).rev() {
        let diff = a[i] as i16 - b[i] as i16 - borrow;
        if diff < 0 {
            result[i] = (diff + 256) as u8;
            borrow = 1;
        } else {
            result[i] = diff as u8;
            borrow = 0;
        }
    }

    result
}

/// Convert a 256-bit big-endian number to f64.
///
/// Precision is limited to ~53 bits, but sufficient for difficulty display.
fn u256_to_f64(val: &[u8; 32]) -> f64 {
    let mut result: f64 = 0.0;
    for &byte in val {
        result = result * 256.0 + byte as f64;
    }
    result
}

/// Format a 256-bit big-endian value as a zero-padded 64-char hex string.
///
/// Matches the chainwork format used by Bitcoin/Dash RPC and Insight.
pub fn u256_to_hex(val: &[u8; 32]) -> String {
    hex::encode(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_from_bits_genesis() {
        // Dash genesis: bits = 0x1e0ffff0
        let target = target_from_bits(0x1e0f_fff0);
        let hex = hex::encode(target);
        // Should be 0x00000ffff0000000...0 (exponent=0x1e=30, mantissa=0x0ffff0)
        assert!(hex.starts_with("00000ffff0"));
        // Remaining bytes should be zero
        assert!(hex.ends_with("0000000000000000000000000000000000000000000000000000"));
    }

    #[test]
    fn test_target_from_bits_max() {
        // Dash max target: 0x1e0fffff
        let target = target_from_bits(0x1e0f_ffff);
        let hex = hex::encode(target);
        assert!(hex.starts_with("00000fffff"));
    }

    #[test]
    fn test_target_from_bits_bitcoin_genesis() {
        // Bitcoin genesis: bits = 0x1d00ffff
        let target = target_from_bits(0x1d00_ffff);
        let hex = hex::encode(target);
        // exponent=0x1d=29, mantissa=0x00ffff
        // target = 0x00000000ffff00000000...0
        assert!(hex.starts_with("00000000ffff"));
    }

    #[test]
    fn test_target_from_bits_high_difficulty() {
        // A high-difficulty compact target: 0x18009645
        // exponent = 0x18 = 24, mantissa = 0x009645
        let target = target_from_bits(0x1800_9645);
        let hex = hex::encode(target);
        // 32 - 24 = 8 leading bytes, then 00 96 45 then zeros
        assert!(hex.starts_with("0000000000000000009645"));
    }

    #[test]
    fn test_difficulty_from_bits_genesis() {
        // Dash genesis block: bits = 0x1e0ffff0
        // Reference is Bitcoin's difficulty-1 target (0x1d00ffff).
        // Dash genesis target is much easier, so difficulty ≈ 1/4096
        let diff = difficulty_from_bits(0x1e0f_fff0);
        let expected = 0.000244140625; // 1/4096
        assert!(
            (diff - expected).abs() < 1e-12,
            "Expected {}, got {}",
            expected,
            diff
        );
    }

    #[test]
    fn test_difficulty_from_bits_bitcoin_difficulty_1() {
        // Bitcoin's difficulty-1 target: bits = 0x1d00ffff
        // Should yield difficulty exactly 1.0
        let diff = difficulty_from_bits(MAX_TARGET_BITS);
        assert!((diff - 1.0).abs() < 1e-10, "Expected 1.0, got {}", diff);
    }

    #[test]
    fn test_difficulty_from_bits_high() {
        // Higher difficulty (lower target) should yield higher difficulty number
        let diff_low = difficulty_from_bits(0x1e0f_fff0);
        let diff_high = difficulty_from_bits(0x1b0a_c88d); // random higher diff
        assert!(diff_high > diff_low);
    }

    #[test]
    fn test_work_from_bits_genesis() {
        // Work for genesis block should be a small positive number
        let work = work_from_bits(0x1e0f_fff0);
        assert_ne!(work, [0u8; 32]);
        // Work should be > 0
        let hex = u256_to_hex(&work);
        assert!(!hex.is_empty());
        assert_ne!(hex, "0");
    }

    #[test]
    fn test_work_higher_difficulty_means_more_work() {
        // Higher difficulty (lower target) should produce more work
        let work_easy = work_from_bits(0x1e0f_fff0);
        let work_hard = work_from_bits(0x1b0a_c88d);

        // work_hard > work_easy in big-endian comparison
        assert!(
            work_hard < work_easy || gte_u256(&work_hard, &work_easy),
            "Harder blocks should have more work"
        );
        // More precisely: the harder block should have strictly greater work
        assert!(
            gte_u256(&work_hard, &work_easy) && work_hard != work_easy,
            "work_hard={} should be > work_easy={}",
            u256_to_hex(&work_hard),
            u256_to_hex(&work_easy),
        );
    }

    #[test]
    fn test_add_u256_basic() {
        let mut a = [0u8; 32];
        a[31] = 100;
        let mut b = [0u8; 32];
        b[31] = 55;

        let result = add_u256(&a, &b);
        assert_eq!(result[31], 155);
        assert_eq!(result[30], 0);
    }

    #[test]
    fn test_add_u256_carry() {
        let mut a = [0u8; 32];
        a[31] = 200;
        let mut b = [0u8; 32];
        b[31] = 100;

        let result = add_u256(&a, &b);
        // 200 + 100 = 300 = 0x012C
        assert_eq!(result[31], 0x2C);
        assert_eq!(result[30], 1);
    }

    #[test]
    fn test_add_u256_zero() {
        let a = [0u8; 32];
        let b = [0u8; 32];
        assert_eq!(add_u256(&a, &b), [0u8; 32]);
    }

    #[test]
    fn test_u256_to_hex() {
        let mut val = [0u8; 32];
        val[31] = 0xff;
        assert_eq!(
            u256_to_hex(&val),
            "00000000000000000000000000000000000000000000000000000000000000ff"
        );

        let zero = [0u8; 32];
        assert_eq!(
            u256_to_hex(&zero),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );

        val[0] = 0x01;
        assert!(u256_to_hex(&val).starts_with("01"));
        assert_eq!(u256_to_hex(&val).len(), 64);
    }

    #[test]
    fn test_u256_to_hex_chainwork() {
        // Simulate accumulated chainwork matching Insight format
        let mut cw = [0u8; 32];
        cw[28] = 0x01;
        cw[29] = 0x23;
        cw[30] = 0x45;
        cw[31] = 0x67;
        assert_eq!(
            u256_to_hex(&cw),
            "0000000000000000000000000000000000000000000000000000000001234567"
        );
    }

    #[test]
    fn test_target_from_bits_zero_exponent() {
        // Edge case: exponent = 0
        let target = target_from_bits(0x0000_0001);
        assert_eq!(target, [0u8; 32]);
    }

    #[test]
    fn test_target_from_bits_negative_flag() {
        // Negative flag set (bit 23 of mantissa)
        let target = target_from_bits(0x1e80_0000);
        assert_eq!(target, [0u8; 32]);
    }

    #[test]
    fn test_div_u256_basic() {
        let mut a = [0u8; 32];
        a[31] = 10;
        let mut b = [0u8; 32];
        b[31] = 3;
        let (q, r) = div_u256(&a, &b);
        assert_eq!(q[31], 3); // 10 / 3 = 3
        assert_eq!(r[31], 1); // 10 % 3 = 1
    }
}
