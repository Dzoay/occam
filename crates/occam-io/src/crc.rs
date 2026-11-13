//! Source: crc-32
//! The CRC-32 the binary twin ends with (docs/decisions/2026-11-13-binary-twin-checksum.md): the one PNG, zip and gzip
//! use, so any tool can check a file. Bits are taken least significant first, against the polynomial 0x04C11DB7 written
//! in that order, 0xEDB88320, from a register that starts all ones and is inverted at the end. Eight bytes are taken at a
//! time through eight tables, each the one before it advanced a byte further, so that loading spends little on it.

/// The polynomial, its bits reversed.
const POLYNOMIAL: u32 = 0xEDB8_8320;

/// Table `k` gives, for a byte, what it adds to the register when `k` more bytes follow it in the same step of eight.
const TABLES: [[u32; 256]; 8] = tables();

/// The eight tables, built when the crate is compiled: the first a byte's remainder, each other one the table before it
/// advanced by a byte of zeros.
#[expect(clippy::indexing_slicing, reason = "a table filled by index, every index below its loop's bound of 256")]
#[expect(clippy::arithmetic_side_effects, reason = "counters that stop at 256, and a table's index less one from 1")]
const fn tables() -> [[u32; 256]; 8] {
    let mut tables = [[0_u32; 256]; 8];
    let mut byte = 0_usize;
    while byte < 256 {
        #[expect(clippy::cast_possible_truncation, reason = "a byte, below 256")]
        let mut register = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            register = if register & 1 == 1 { (register >> 1) ^ POLYNOMIAL } else { register >> 1 };
            bit += 1;
        }
        tables[0][byte] = register;
        byte += 1;
    }
    let mut table = 1;
    while table < 8 {
        let mut byte = 0;
        while byte < 256 {
            let before = tables[table - 1][byte];
            tables[table][byte] = (before >> 8) ^ tables[0][(before & 0xFF) as usize];
            byte += 1;
        }
        table += 1;
    }
    tables
}

/// What `table` gives for `byte`.
fn at(table: &[u32; 256], byte: u8) -> u32 {
    table.get(usize::from(byte)).copied().unwrap_or(0)
}

/// The CRC-32 of `bytes`.
pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let [t0, t1, t2, t3, t4, t5, t6, t7] = &TABLES;
    let (eights, rest) = bytes.as_chunks::<8>();
    let mut register = u32::MAX;
    for &[a, b, c, d, e, f, g, h] in eights {
        let [a, b, c, d] = (u32::from_le_bytes([a, b, c, d]) ^ register).to_le_bytes();
        register = at(t7, a) ^ at(t6, b) ^ at(t5, c) ^ at(t4, d) ^ at(t3, e) ^ at(t2, f) ^ at(t1, g) ^ at(t0, h);
    }
    for &byte in rest {
        let [low, ..] = register.to_le_bytes();
        register = at(t0, low ^ byte) ^ (register >> 8);
    }
    !register
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CRC-32 of `bytes` a bit at a time, as its definition gives it, with no table.
    fn by_bits(bytes: &[u8]) -> u32 {
        let mut register = u32::MAX;
        for &byte in bytes {
            register ^= u32::from(byte);
            for _ in 0..8 {
                register = if register & 1 == 1 { (register >> 1) ^ POLYNOMIAL } else { register >> 1 };
            }
        }
        !register
    }

    #[test]
    fn the_published_check_values_come_out() {
        // The check value every CRC-32 catalogue gives, and two more that tools such as `crc32` and zlib print.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn eight_bytes_at_a_time_give_what_one_bit_at_a_time_gives_at_every_length_and_byte() {
        // Every byte value at every place of an eight-byte step, and every length round a step's ends.
        let bytes: Vec<u8> = (0_u32..2048).map(|i| u8::try_from((i * 167 + i / 256) % 256).unwrap()).collect();
        for length in 0..=40 {
            let part = bytes.get(..length).unwrap();
            assert_eq!(crc32(part), by_bits(part), "{length} bytes");
        }
        assert_eq!(crc32(&bytes), by_bits(&bytes));
        for byte in 0..=255_u8 {
            for place in 0..16 {
                let mut one = vec![0_u8; 16];
                *one.get_mut(place).unwrap() = byte;
                assert_eq!(crc32(&one), by_bits(&one), "{byte} at {place}");
            }
        }
    }
}
