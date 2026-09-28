//! Bit numbering, per-signal bit layouts and raw bit I/O.
//!
//! Bits are numbered from the least significant bit of the first byte:
//! byte 0 holds bits 0..=7 (bit 0 = LSB), byte 1 holds bits 8..=15, and so
//! on.
//!
//! * Intel/little-endian: the start bit holds the least significant bit of
//!   the value; successive value bits occupy start+1, start+2, ...
//! * Motorola/big-endian: the start bit holds the most significant bit;
//!   successive bits step down by 1, and after the low bit of a byte
//!   (position % 8 == 0) the next step adds 15, landing on the high bit of
//!   the following byte.

use crate::schema::EndianDef;

/// Compiled byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Intel,
    Motorola,
}

impl From<EndianDef> for Endian {
    fn from(e: EndianDef) -> Self {
        match e {
            EndianDef::Intel => Endian::Intel,
            EndianDef::Motorola => Endian::Motorola,
        }
    }
}

/// Expand a signal definition into the ordered list of occupied bit positions.
///
/// Positions are returned from the most significant value bit toward the least
/// significant value bit, for both byte orders. Every position is guaranteed
/// to be `< total_bits`.
pub fn layout_positions(
    endian: Endian,
    start_bit: u16,
    width: u8,
    total_bits: u16,
) -> Option<Vec<u16>> {
    if width == 0 || start_bit >= total_bits {
        return None;
    }
    let mut positions = Vec::with_capacity(width as usize);
    match endian {
        Endian::Intel => {
            for k in 0..width as u16 {
                let p = start_bit.checked_add(k)?;
                if p >= total_bits {
                    return None;
                }
                positions.push(p);
            }
            // Currently LSB-first; reverse to MSB-first for uniform handling.
            positions.reverse();
        }
        Endian::Motorola => {
            let mut p = start_bit;
            for k in 0..width {
                if p >= total_bits {
                    return None;
                }
                positions.push(p);
                if k + 1 < width {
                    if p % 8 == 0 {
                        p = p.checked_add(15)?;
                    } else {
                        p -= 1;
                    }
                }
            }
        }
    }
    Some(positions)
}

/// Read `width` bits at the given MSB-first positions into a raw integer.
pub fn read_bits(data: &[u8], positions: &[u16]) -> u32 {
    let mut raw: u32 = 0;
    for &pos in positions {
        raw <<= 1;
        let byte = data[(pos / 8) as usize];
        let bit = (pos % 8) as u8;
        raw |= ((byte >> bit) & 1) as u32;
    }
    raw
}

/// Write `width` low bits of `raw` at the given MSB-first positions.
pub fn write_bits(data: &mut [u8], positions: &[u16], raw: u32) {
    for (i, &pos) in positions.iter().enumerate() {
        let value_bit = (positions.len() - 1 - i) as u32;
        let bit_value = ((raw >> value_bit) & 1) as u8;
        let byte = &mut data[(pos / 8) as usize];
        let bit = (pos % 8) as u8;
        *byte = (*byte & !(1 << bit)) | (bit_value << bit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intel_layout_is_lsb_first() {
        let pos = layout_positions(Endian::Intel, 0, 8, 16).unwrap();
        assert_eq!(pos, vec![7, 6, 5, 4, 3, 2, 1, 0]);
        let cross = layout_positions(Endian::Intel, 6, 4, 16).unwrap();
        assert_eq!(cross, vec![9, 8, 7, 6]);
    }

    #[test]
    fn motorola_layout_walks_backward_then_jumps() {
        let cross = layout_positions(Endian::Motorola, 7, 12, 16).unwrap();
        assert_eq!(cross, vec![7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12]);
    }

    #[test]
    fn roundtrip_bits() {
        let mut data = [0u8; 4];
        let pos = layout_positions(Endian::Motorola, 23, 16, 32).unwrap();
        write_bits(&mut data, &pos, 0xABCD);
        assert_eq!(read_bits(&data, &pos), 0xABCD);
        assert_eq!(&data[..4], &[0x00, 0x00, 0xAB, 0xCD]);
    }

    #[test]
    fn rejects_out_of_bounds() {
        assert!(layout_positions(Endian::Intel, 60, 8, 64).is_none());
        assert!(layout_positions(Endian::Motorola, 60, 8, 64).is_none());
        assert!(layout_positions(Endian::Intel, 63, 2, 64).is_none());
    }
}
