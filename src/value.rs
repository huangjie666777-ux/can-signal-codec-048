use crate::compiler::Signal;
use crate::error::CodecError;

pub(crate) fn physical_to_raw(signal: &Signal, physical: f64) -> Result<u32, CodecError> {
    if !physical.is_finite() {
        return Err(CodecError::Invalid(format!(
            "signal '{}' physical value must be finite",
            signal.name
        )));
    }

    let raw_float = (physical - signal.offset) / signal.factor;
    if !raw_float.is_finite() {
        return Err(CodecError::Invalid(format!(
            "signal '{}' physical value produces a non-finite raw value",
            signal.name
        )));
    }
    let rounded = raw_float.round();
    let (minimum, maximum) = raw_bounds(signal.signed, signal.width);
    if rounded < minimum || rounded > maximum {
        return Err(CodecError::Invalid(format!(
            "signal '{}' raw value {} is outside the {} {}-bit range",
            signal.name,
            rounded as i64,
            if signal.signed { "signed" } else { "unsigned" },
            signal.width
        )));
    }

    let signed_raw = rounded as i64;
    Ok(if signal.signed {
        signed_raw as u32
    } else {
        u32::try_from(signed_raw).expect("unsigned range was validated")
    })
}

pub(crate) fn raw_to_physical(signal: &Signal, raw: u32) -> Result<f64, CodecError> {
    let numeric = if signal.signed {
        sign_extend(raw, signal.width) as f64
    } else {
        f64::from(raw)
    };
    let physical = numeric.mul_add(signal.factor, signal.offset);
    if !physical.is_finite() {
        return Err(CodecError::Invalid(format!(
            "signal '{}' raw value produces a non-finite physical value",
            signal.name
        )));
    }
    Ok(physical)
}

fn raw_bounds(signed: bool, width: u8) -> (f64, f64) {
    if signed {
        let negative_limit = 2f64.powi(i32::from(width) - 1);
        (-negative_limit, negative_limit - 1.0)
    } else {
        (0.0, 2f64.powi(i32::from(width)) - 1.0)
    }
}

fn sign_extend(raw: u32, width: u8) -> i32 {
    let sign_bit = 1u32 << (u32::from(width) - 1);
    let bits = if width == 32 {
        u32::MAX
    } else {
        (1u32 << u32::from(width)) - 1
    };
    if raw & sign_bit != 0 {
        (raw | !bits) as i32
    } else {
        (raw & bits) as i32
    }
}
