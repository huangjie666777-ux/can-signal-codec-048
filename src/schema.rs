//! Raw JSON schema structures, deserialized before validation/compilation.

use serde::Deserialize;

/// Top-level JSON document: `{ "frames": [ ... ] }`.
#[derive(Debug, Deserialize)]
pub struct Schema {
    pub frames: Vec<FrameDef>,
}

/// JSON definition of one CAN frame.
#[derive(Debug, Deserialize)]
pub struct FrameDef {
    /// Human-readable, unique frame name.
    pub name: String,
    /// CAN arbitration ID (11-bit standard, or 29-bit extended).
    pub id: u32,
    /// `true` for a 29-bit extended ID, `false` for an 11-bit standard ID.
    #[serde(default)]
    pub extended: bool,
    /// Payload length in bytes, 1..=8.
    pub length: u8,
    pub signals: Vec<SignalDef>,
}

/// Byte order as written in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndianDef {
    Intel,
    Motorola,
}

impl<'de> Deserialize<'de> for EndianDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "intel" | "little" | "little_endian" | "lsb" => Ok(EndianDef::Intel),
            "motorola" | "big" | "big_endian" | "msb" => Ok(EndianDef::Motorola),
            other => Err(serde::de::Error::custom(format!(
                "unknown endian `{other}` (expected intel or motorola)"
            ))),
        }
    }
}

/// JSON definition of one signal.
#[derive(Debug, Deserialize)]
pub struct SignalDef {
    /// Unique signal name within the frame.
    pub name: String,
    /// Start bit. Bit 0 is the LSB of the first byte; bits are numbered
    /// consecutively across bytes.
    pub start_bit: u16,
    /// Bit width, 1..=32.
    pub width: u8,
    pub endian: EndianDef,
    #[serde(default)]
    pub signed: bool,
    #[serde(default = "default_factor")]
    pub factor: f64,
    #[serde(default)]
    pub offset: f64,
    /// Marks the (at most one) unsigned multiplex/selector signal
    /// (factor 1, offset 0).
    #[serde(default)]
    pub selector: bool,
    /// When present, the signal is active only while the selector's raw value
    /// equals this integer.
    #[serde(default)]
    pub when: Option<i64>,
}

fn default_factor() -> f64 {
    1.0
}
