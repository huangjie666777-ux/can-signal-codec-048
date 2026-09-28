//! CAN signal frame encoding and decoding.

mod bits;
mod branch;
mod codec;
mod compiler;
mod error;
mod schema;
mod value;

pub use codec::{CanMessage, Database, DecodedSignal, EncodedFrame, Frame};
pub use error::{CodecError, DefinitionError};
pub use schema::{ByteOrder, Condition, SignalDefinition};

use std::collections::BTreeMap;

/// Physical values keyed by signal name.
pub type PhysicalValues = BTreeMap<String, f64>;

/// Decoded signal values keyed by signal name.
pub type DecodedSignals = BTreeMap<String, DecodedSignal>;
