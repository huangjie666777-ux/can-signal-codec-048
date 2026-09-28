//! Physical <-> raw numeric conversion.
//!
//! `physical = raw * factor + offset`. Inverse encoding rounds to the nearest
//! raw integer, with exact midpoints rounded away from zero.

/// Round half away from zero, returning an `i64` when it fits.
pub fn round_half_away(x: f64) -> Option<i64> {
    if !x.is_finite() {
        return None;
    }
    let rounded = if x >= 0.0 {
        (x + 0.5).floor()
    } else {
        -((-x + 0.5).floor())
    };
    if rounded.is_finite() && rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64 {
        Some(rounded as i64)
    } else {
        None
    }
}

/// Convert a physical value to a signed raw integer by inverting factor/offset.
pub fn physical_to_raw(physical: f64, factor: f64, offset: f64) -> Option<i64> {
    if !physical.is_finite() {
        return None;
    }
    round_half_away((physical - offset) / factor)
}

/// Convert a raw unsigned bit pattern to its physical value.
pub fn raw_to_physical(raw: i64, factor: f64, offset: f64) -> f64 {
    raw as f64 * factor + offset
}

/// Sign-extend a `width`-bit unsigned pattern to a signed `i64`.
pub fn reinterpret_signed(raw: u32, width: u8) -> i64 {
    let shift = 32 - width as u32;
    ((raw << shift) as i32 >> shift) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_is_nearest_half_away_from_zero() {
        assert_eq!(round_half_away(2.5), Some(3));
        assert_eq!(round_half_away(-2.5), Some(-3));
        assert_eq!(round_half_away(2.4), Some(2));
        assert_eq!(round_half_away(-2.4), Some(-2));
        assert_eq!(round_half_away(f64::NAN), None);
        assert_eq!(round_half_away(f64::INFINITY), None);
    }

    #[test]
    fn physical_math_roundtrip() {
        let raw = physical_to_raw(12.5, 0.1, 0.5).unwrap();
        assert_eq!(raw, 120);
        let phys = raw_to_physical(raw, 0.1, 0.5);
        assert!((phys - 12.5).abs() < 1e-12);
    }

    #[test]
    fn sign_extension() {
        assert_eq!(reinterpret_signed(0b1001, 4), -7);
        assert_eq!(reinterpret_signed(0b0111, 4), 7);
        assert_eq!(reinterpret_signed(0xFFFF_FFFF, 32), -1);
    }
}
