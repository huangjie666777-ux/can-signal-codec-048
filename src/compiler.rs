use std::collections::{BTreeMap, HashSet};

use crate::bits::{BitLayout, MAX_SIGNAL_WIDTH};
use crate::error::DefinitionError;
use crate::schema::{DatabaseJson, FrameJson, SignalDefinition};

#[derive(Debug, Clone)]
pub(crate) struct Signal {
    pub(crate) name: String,
    pub(crate) width: u8,
    pub(crate) signed: bool,
    pub(crate) factor: f64,
    pub(crate) offset: f64,
    pub(crate) is_selector: bool,
    pub(crate) condition: Option<(String, u32)>,
    pub(crate) layout: BitLayout,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledFrame {
    pub(crate) id: u32,
    pub(crate) extended: bool,
    pub(crate) dlc: u8,
    pub(crate) signals: Vec<Signal>,
    pub(crate) selector_index: Option<usize>,
}

impl CompiledFrame {
    pub(crate) fn signal_by_name(&self, name: &str) -> Option<&Signal> {
        self.signals.iter().find(|signal| signal.name == name)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledDatabase {
    pub(crate) frames: BTreeMap<(u32, bool), CompiledFrame>,
}

pub(crate) fn compile_json(json: &DatabaseJson) -> Result<CompiledDatabase, DefinitionError> {
    let mut frames = BTreeMap::new();
    for frame in &json.frames {
        let compiled = compile_frame(frame)?;
        let key = (compiled.id, compiled.extended);
        if frames.insert(key, compiled).is_some() {
            let identity = frame_identity(frame.id, frame.extended);
            return Err(DefinitionError::Invalid(format!(
                "duplicate frame definition for {identity}"
            )));
        }
    }
    Ok(CompiledDatabase { frames })
}

fn compile_frame(frame: &FrameJson) -> Result<CompiledFrame, DefinitionError> {
    let max_id = if frame.extended { 0x1FFF_FFFF } else { 0x7FF };
    if frame.id > max_id {
        return Err(DefinitionError::Invalid(format!(
            "{} ID 0x{:X} is outside its allowed range",
            frame_identity(frame.id, frame.extended),
            frame.id
        )));
    }
    if !(1..=8).contains(&frame.dlc) {
        return Err(DefinitionError::Invalid(format!(
            "frame 0x{:X} DLC {} is outside the 1..=8 range",
            frame.id, frame.dlc
        )));
    }
    if frame.signals.is_empty() {
        return Err(DefinitionError::Invalid(format!(
            "frame 0x{:X} must contain at least one signal",
            frame.id
        )));
    }

    let frame_bits = u16::from(frame.dlc) * 8;
    let mut names = HashSet::new();
    let mut signals = Vec::with_capacity(frame.signals.len());
    let mut selector_index = None;

    for (index, definition) in frame.signals.iter().enumerate() {
        let signal = compile_signal(frame, definition, frame_bits)?;
        if !names.insert(signal.name.clone()) {
            return Err(DefinitionError::Invalid(format!(
                "frame 0x{:X} contains duplicate signal name '{}'",
                frame.id, signal.name
            )));
        }
        if signal.is_selector {
            if selector_index.is_some() {
                return Err(DefinitionError::Invalid(format!(
                    "frame 0x{:X} contains more than one selector",
                    frame.id
                )));
            }
            selector_index = Some(index);
        }
        signals.push(signal);
    }

    let selector_name = selector_index.map(|index| signals[index].name.clone());
    for signal in &signals {
        if let Some((condition_selector, value)) = &signal.condition {
            let selector = match selector_name {
                Some(ref name) if name == condition_selector => signals
                    .iter()
                    .find(|candidate| &candidate.name == name)
                    .expect("selector name was validated"),
                _ => {
                    return Err(DefinitionError::Invalid(format!(
                        "signal '{}' refers to nonexistent selector '{}'",
                        signal.name, condition_selector
                    )));
                }
            };
            if u64::from(*value) >= selector_max_raw(selector.width) {
                return Err(DefinitionError::Invalid(format!(
                    "signal '{}' condition value {} is outside selector range",
                    signal.name, value
                )));
            }
        }
    }

    for left_index in 0..signals.len() {
        for right_index in (left_index + 1)..signals.len() {
            if signals_can_overlap(&signals[left_index], &signals[right_index])
                && positions_overlap(
                    signals[left_index].layout.positions(),
                    signals[right_index].layout.positions(),
                )
            {
                return Err(DefinitionError::Invalid(format!(
                    "signals '{}' and '{}' overlap while simultaneously activatable",
                    signals[left_index].name, signals[right_index].name
                )));
            }
        }
    }

    Ok(CompiledFrame {
        id: frame.id,
        extended: frame.extended,
        dlc: frame.dlc,
        signals,
        selector_index,
    })
}

fn compile_signal(
    frame: &FrameJson,
    definition: &SignalDefinition,
    frame_bits: u16,
) -> Result<Signal, DefinitionError> {
    if definition.name.trim().is_empty() {
        return Err(DefinitionError::Invalid(format!(
            "frame 0x{:X} contains a signal with an empty name",
            frame.id
        )));
    }
    if !(1..=MAX_SIGNAL_WIDTH).contains(&definition.width) {
        return Err(DefinitionError::Invalid(format!(
            "signal '{}' width {} is outside the 1..=32 range",
            definition.name, definition.width
        )));
    }
    if !definition.factor.is_finite() {
        return Err(DefinitionError::Invalid(format!(
            "signal '{}' factor must be finite",
            definition.name
        )));
    }
    if definition.factor == 0.0 {
        return Err(DefinitionError::Invalid(format!(
            "signal '{}' factor must be nonzero",
            definition.name
        )));
    }
    if !definition.offset.is_finite() {
        return Err(DefinitionError::Invalid(format!(
            "signal '{}' offset must be finite",
            definition.name
        )));
    }
    if definition.is_selector {
        if definition.signed {
            return Err(DefinitionError::Invalid(format!(
                "selector '{}' must be unsigned",
                definition.name
            )));
        }
        if definition.factor != 1.0 || definition.offset != 0.0 {
            return Err(DefinitionError::Invalid(format!(
                "selector '{}' must have factor 1 and offset 0",
                definition.name
            )));
        }
        if definition.condition.is_some() {
            return Err(DefinitionError::Invalid(format!(
                "selector '{}' cannot itself be conditional",
                definition.name
            )));
        }
    }

    let layout = BitLayout::compile(
        definition.start_bit,
        definition.width,
        definition.byte_order,
        frame_bits,
    )
    .map_err(|message| {
        DefinitionError::Invalid(format!("signal '{}': {message}", definition.name))
    })?;
    let condition = definition
        .condition
        .as_ref()
        .map(|condition| (condition.selector.clone(), condition.selector_value));

    Ok(Signal {
        name: definition.name.clone(),
        width: definition.width,
        signed: definition.signed,
        factor: definition.factor,
        offset: definition.offset,
        is_selector: definition.is_selector,
        condition,
        layout,
    })
}

fn selector_max_raw(width: u8) -> u64 {
    1u64.wrapping_shl(u32::from(width))
}

fn signals_can_overlap(left: &Signal, right: &Signal) -> bool {
    match (&left.condition, &right.condition) {
        (None, _) | (_, None) => true,
        (Some(left_condition), Some(right_condition)) => left_condition.0 != right_condition.0,
    }
}

fn positions_overlap(left: &[u16], right: &[u16]) -> bool {
    left.iter().any(|position| right.contains(position))
}

pub(crate) fn frame_identity(id: u32, extended: bool) -> String {
    let kind = if extended {
        "extended frame"
    } else {
        "standard frame"
    };
    format!("{kind} 0x{id:X}")
}
