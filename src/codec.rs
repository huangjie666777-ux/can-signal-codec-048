use std::collections::BTreeMap;
use std::path::Path;

use crate::branch::ActiveSet;
use crate::compiler::{compile_json, CompiledDatabase, CompiledFrame};
use crate::error::{CodecError, DefinitionError};
use crate::schema::DatabaseJson;
use crate::value::{physical_to_raw, raw_to_physical};
use crate::PhysicalValues;

/// A CAN frame identity and payload as found on a bus boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanMessage {
    /// Arbitration ID.
    pub id: u32,
    /// True for an extended 29-bit ID, false for an 11-bit standard ID.
    pub extended: bool,
    /// Payload bytes. Length must equal the frame definition DLC.
    pub data: Vec<u8>,
}

/// Bytes produced by encoding a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    /// Frame ID and extended-frame flag.
    pub message: CanMessage,
}

impl EncodedFrame {
    /// Payload bytes, one through eight bytes long.
    pub fn data(&self) -> &[u8] {
        &self.message.data
    }
}

/// Raw and physical values of one decoded signal.
#[derive(Debug, Clone, Copy)]
pub struct DecodedSignal {
    /// Raw unsigned representation; signed values are the two-complement bit pattern.
    pub raw: u32,
    /// Physical value after factor and offset conversion.
    pub physical: f64,
}

/// A validated, compiled frame definition.
#[derive(Debug)]
pub struct Frame<'a> {
    compiled: &'a CompiledFrame,
}

impl Frame<'_> {
    /// CAN arbitration ID.
    pub fn id(&self) -> u32 {
        self.compiled.id
    }

    /// True when the ID uses extended 29-bit arbitration.
    pub fn extended(&self) -> bool {
        self.compiled.extended
    }

    /// Required payload length.
    pub fn dlc(&self) -> u8 {
        self.compiled.dlc
    }

    /// Encode physical signal values for this frame.
    pub fn encode(&self, values: &PhysicalValues) -> Result<EncodedFrame, CodecError> {
        encode_frame(self.compiled, values)
    }

    /// Decode a matching CAN message using this frame definition.
    pub fn decode(&self, message: &CanMessage) -> Result<crate::DecodedSignals, CodecError> {
        decode_frame(self.compiled, message)
    }
}

/// An immutable collection of compiled frame definitions.
#[derive(Debug)]
pub struct Database {
    compiled: CompiledDatabase,
}

impl Database {
    /// Compile definitions from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, DefinitionError> {
        let parsed: DatabaseJson = serde_json::from_str(json)?;
        let compiled = compile_json(&parsed)?;
        Ok(Self { compiled })
    }

    /// Compile definitions from JSON bytes.
    pub fn from_json_bytes(json: &[u8]) -> Result<Self, DefinitionError> {
        let parsed: DatabaseJson = serde_json::from_slice(json)?;
        let compiled = compile_json(&parsed)?;
        Ok(Self { compiled })
    }

    /// Read and compile definitions from a JSON file.
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, DefinitionError> {
        let bytes = std::fs::read(path).map_err(|error| {
            DefinitionError::Invalid(format!("could not read definition file: {error}"))
        })?;
        Self::from_json_bytes(&bytes)
    }

    /// Look up a frame by its arbitration ID and standard or extended identity.
    pub fn frame(&self, id: u32, extended: bool) -> Option<Frame<'_>> {
        self.compiled
            .frames
            .get(&(id, extended))
            .map(|compiled| Frame { compiled })
    }

    /// Encode physical values for a frame selected by ID and ID type.
    pub fn encode_frame(
        &self,
        id: u32,
        extended: bool,
        values: &PhysicalValues,
    ) -> Result<EncodedFrame, CodecError> {
        let frame = self.lookup_frame(id, extended)?;
        frame.encode(values)
    }

    /// Decode a CAN message after checking ID, ID type, and payload length.
    pub fn decode(&self, message: &CanMessage) -> Result<crate::DecodedSignals, CodecError> {
        let frame = self.lookup_frame(message.id, message.extended)?;
        frame.decode(message)
    }

    fn lookup_frame(&self, id: u32, extended: bool) -> Result<Frame<'_>, CodecError> {
        self.frame(id, extended).ok_or_else(|| {
            CodecError::Invalid(format!(
                "no definition for {} 0x{id:X}",
                if extended {
                    "extended frame"
                } else {
                    "standard frame"
                }
            ))
        })
    }
}

fn encode_frame(
    frame: &CompiledFrame,
    values: &PhysicalValues,
) -> Result<EncodedFrame, CodecError> {
    for name in values.keys() {
        if frame.signal_by_name(name).is_none() {
            return Err(CodecError::Invalid(format!(
                "unknown signal '{name}' for frame 0x{:X}",
                frame.id
            )));
        }
    }

    let active = match frame.selector_index {
        Some(_) => ActiveSet::from_encoded_values(frame, values)?,
        None => ActiveSet::with_selector(frame, None),
    };

    let mut missing = active.missing(values);
    missing.sort();
    if let Some(name) = missing.first() {
        return Err(CodecError::Invalid(format!(
            "missing active signal '{name}'"
        )));
    }
    for name in values.keys() {
        if !active.contains(name) {
            return Err(CodecError::Invalid(format!(
                "inactive signal '{name}' must not be supplied"
            )));
        }
    }

    let mut data = vec![0u8; usize::from(frame.dlc)];
    for signal in &frame.signals {
        if !active.contains(&signal.name) {
            continue;
        }
        let physical = values[&signal.name];
        let raw = physical_to_raw(signal, physical)?;
        signal.layout.write_raw(&mut data, raw);
    }

    Ok(EncodedFrame {
        message: CanMessage {
            id: frame.id,
            extended: frame.extended,
            data,
        },
    })
}

fn decode_frame(
    frame: &CompiledFrame,
    message: &CanMessage,
) -> Result<BTreeMap<String, DecodedSignal>, CodecError> {
    if message.id != frame.id || message.extended != frame.extended {
        return Err(CodecError::Invalid(
            "message identity does not match frame".to_string(),
        ));
    }
    if message.data.len() != usize::from(frame.dlc) {
        return Err(CodecError::Invalid(format!(
            "frame 0x{:X} expects {} payload bytes but received {}",
            frame.id,
            frame.dlc,
            message.data.len()
        )));
    }

    let selector_raw = match frame.selector_index {
        Some(index) => Some(frame.signals[index].layout.read_raw(&message.data)),
        None => None,
    };
    let active = ActiveSet::with_selector(frame, selector_raw);
    let mut decoded = BTreeMap::new();
    for signal in &frame.signals {
        if !active.contains(&signal.name) {
            continue;
        }
        let raw = signal.layout.read_raw(&message.data);
        let physical = raw_to_physical(signal, raw)?;
        decoded.insert(signal.name.clone(), DecodedSignal { raw, physical });
    }
    Ok(decoded)
}
