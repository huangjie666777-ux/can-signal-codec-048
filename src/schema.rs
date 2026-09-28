use serde::{Deserialize, Serialize};

/// Wire bit numbering supported by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ByteOrder {
    /// Little-endian signal; the start bit is its least-significant bit.
    Intel,
    /// Big-endian signal using the Motorola saw-tooth bit walk.
    Motorola,
}

/// Selector value required for a conditional signal to be active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// Name of the frame's unsigned selector signal.
    pub selector: String,
    /// Raw selector integer that activates this signal.
    #[serde(rename = "value")]
    pub selector_value: u32,
}

/// JSON representation of one signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalDefinition {
    /// Unique signal name within its frame.
    pub name: String,
    /// First bit in the signal, continuous numbering from byte 0 bit 0.
    pub start_bit: u8,
    /// Signal width in bits (1 through 32).
    pub width: u8,
    /// Intel or Motorola bit order.
    pub byte_order: ByteOrder,
    /// Whether the raw integer is two's complement signed.
    pub signed: bool,
    /// Physical-value multiplier; must be finite and nonzero.
    pub factor: f64,
    /// Physical-value offset; must be finite.
    pub offset: f64,
    /// Marks the single unsigned branch selector.
    #[serde(default)]
    pub is_selector: bool,
    /// When present, this signal is active only for this raw selector value.
    #[serde(default)]
    pub condition: Option<Condition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameJson {
    pub(crate) id: u32,
    #[serde(default)]
    pub(crate) extended: bool,
    pub(crate) dlc: u8,
    pub(crate) signals: Vec<SignalDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DatabaseJson {
    pub(crate) frames: Vec<FrameJson>,
}
