//! Public encode/decode API.

use std::collections::{HashMap, HashSet};

use crate::bits::{read_bits, write_bits};
use crate::compile::{CompiledFrame, CompiledSignal, Database};
use crate::convert::{physical_to_raw, raw_to_physical, reinterpret_signed};
use crate::error::{Error, Result};

/// One decoded signal: raw integer plus scaled physical value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecodedSignal {
    pub raw: i64,
    pub physical: f64,
}

impl DecodedSignal {
    pub fn new(raw: i64, physical: f64) -> Self {
        Self { raw, physical }
    }
}

impl Database {
    /// Encode a frame from physical signal values.
    ///
    /// `values` must contain exactly the signals active for the selected
    /// branch: the selector (if any), every resident signal, and the signals
    /// bound to that selector value. Unknown, missing and inactive signal
    /// names are rejected. All unused payload bits are emitted as zero.
    pub fn encode(&self, frame_name: &str, values: &[(&str, f64)]) -> Result<CanFrame> {
        let frame = self
            .frame_by_name(frame_name)
            .ok_or_else(|| Error::encode(frame_name, "unknown frame"))?;
        encode_frame(frame, values)
    }

    /// Decode a received frame by CAN id, identity flags and payload.
    ///
    /// Returns a name -> [`DecodedSignal`] map containing exactly the signals
    /// active for the selector value carried in the payload.
    pub fn decode(
        &self,
        id: u32,
        extended: bool,
        data: &[u8],
    ) -> Result<HashMap<String, DecodedSignal>> {
        let frame = self
            .frame_by_id(id)
            .ok_or_else(|| Error::decode(format!("unknown CAN id 0x{id:X}")))?;
        if frame.extended != extended {
            return Err(Error::decode(format!(
                "frame 0x{id:X} identity mismatch (expected extended={}, got extended={extended})",
                frame.extended
            )));
        }
        decode_frame(frame, data)
    }
}

/// A frame ready to put on the bus (the library never sends it itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanFrame {
    pub id: u32,
    pub extended: bool,
    pub data: Vec<u8>,
}

impl CanFrame {
    pub fn new(id: u32, extended: bool, data: Vec<u8>) -> Self {
        Self { id, extended, data }
    }

    /// Render payload as a space-separated hex string, e.g. `0A 1F`.
    pub fn data_hex(&self) -> String {
        self.data
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn encode_frame(frame: &CompiledFrame, values: &[(&str, f64)]) -> Result<CanFrame> {
    let provided: HashMap<&str, f64> = {
        let mut map = HashMap::new();
        for (name, value) in values {
            if map.insert(*name, *value).is_some() {
                return Err(Error::encode(
                    &frame.name,
                    format!("signal `{name}` given more than once"),
                ));
            }
        }
        map
    };

    // Reject unknown names up front.
    for name in provided.keys() {
        if frame.signal_index(name).is_none() {
            return Err(Error::encode(
                &frame.name,
                format!("unknown signal `{name}`"),
            ));
        }
    }

    // Resolve the selector raw value first; it decides which branch is active.
    let selector_raw = if let Some(sel) = frame.selector() {
        let phys = *provided.get(sel.name.as_str()).ok_or_else(|| {
            Error::encode(
                &frame.name,
                format!("missing selector signal `{}`", sel.name),
            )
        })?;
        Some(encode_signal_raw(&frame.name, sel, phys)?)
    } else {
        None
    };

    let active = frame.active_indices(selector_raw);
    let active_names: HashSet<&str> = active
        .iter()
        .map(|&i| frame.signals()[i].name.as_str())
        .collect();

    for name in provided.keys() {
        if !active_names.contains(name) {
            return Err(Error::encode(
                &frame.name,
                format!("signal `{name}` is inactive for this selector value"),
            ));
        }
    }
    for &i in &active {
        let sig = &frame.signals()[i];
        if !provided.contains_key(sig.name.as_str()) {
            return Err(Error::encode(
                &frame.name,
                format!("missing active signal `{}`", sig.name),
            ));
        }
    }

    let mut data = vec![0u8; frame.length as usize];
    for &i in &active {
        let sig = &frame.signals()[i];
        let phys = provided[sig.name.as_str()];
        let raw_i64 = encode_signal_raw(&frame.name, sig, phys)?;
        write_bits(&mut data, &sig.positions, raw_i64 as u32);
    }

    Ok(CanFrame {
        id: frame.id,
        extended: frame.extended,
        data,
    })
}

fn encode_signal_raw(frame_name: &str, sig: &CompiledSignal, physical: f64) -> Result<i64> {
    let raw = physical_to_raw(physical, sig.factor, sig.offset).ok_or_else(|| {
        Error::encode(
            frame_name,
            format!(
                "physical value {} for `{}` is not finite or cannot be inverted",
                physical, sig.name
            ),
        )
    })?;
    let (min, max) = sig.raw_range();
    if raw < min || raw > max {
        return Err(Error::encode(
            frame_name,
            format!(
                "physical value {} for `{}` yields raw {raw}, outside range {min}..={max}",
                physical, sig.name
            ),
        ));
    }
    Ok(raw)
}

fn decode_frame(frame: &CompiledFrame, data: &[u8]) -> Result<HashMap<String, DecodedSignal>> {
    if data.len() != frame.length as usize {
        return Err(Error::decode(format!(
            "frame `{}` expects {} payload bytes, got {}",
            frame.name,
            frame.length,
            data.len()
        )));
    }

    let selector_raw = frame
        .selector()
        .map(|sel| read_bits(data, &sel.positions) as i64);

    let mut out = HashMap::new();
    for &i in frame.active_indices(selector_raw).iter() {
        let sig = &frame.signals()[i];
        let bits = read_bits(data, &sig.positions);
        let raw = if sig.signed {
            reinterpret_signed(bits, sig.width)
        } else {
            bits as i64
        };
        let physical = raw_to_physical(raw, sig.factor, sig.offset);
        if !physical.is_finite() {
            return Err(Error::decode(format!(
                "non-finite physical value decoding signal `{}`",
                sig.name
            )));
        }
        out.insert(sig.name.clone(), DecodedSignal { raw, physical });
    }
    Ok(out)
}
