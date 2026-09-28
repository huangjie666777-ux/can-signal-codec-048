//! Embedded-friendly CAN signal encode/decode library.
//!
//! Load frame definitions from JSON with [`Database::from_json`], encode
//! physical values into CAN payloads with [`Database::encode`], and decode
//! received payloads with [`Database::decode`].
//!
//! See the repository README for the JSON format and a worked example. The
//! crate performs no bus I/O and no DBC parsing.

pub mod bits;
pub mod codec;
pub mod compile;
pub mod convert;
pub mod error;
pub mod schema;

pub use codec::{CanFrame, DecodedSignal};
pub use compile::{CompiledFrame, CompiledSignal, Database};
pub use error::Error;
pub use error::Result;
