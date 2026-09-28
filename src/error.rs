//! Error types for definition loading, encoding and decoding.

use std::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// A single, fully validated error. No partially compiled definitions are
/// ever returned.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The JSON text could not be parsed.
    Json(String),
    /// A frame definition was invalid; `frame` is `None` for top-level errors.
    FrameDef {
        frame: Option<String>,
        message: String,
    },
    /// A signal definition was invalid.
    SignalDef {
        frame: String,
        signal: String,
        message: String,
    },
    /// Encoding failed.
    Encode { frame: String, message: String },
    /// Decoding failed.
    Decode { message: String },
}

impl Error {
    pub(crate) fn frame_def(frame: Option<&str>, message: impl Into<String>) -> Self {
        Error::FrameDef {
            frame: frame.map(str::to_string),
            message: message.into(),
        }
    }

    pub(crate) fn signal_def(frame: &str, signal: &str, message: impl Into<String>) -> Self {
        Error::SignalDef {
            frame: frame.to_string(),
            signal: signal.to_string(),
            message: message.into(),
        }
    }

    pub(crate) fn encode(frame: &str, message: impl Into<String>) -> Self {
        Error::Encode {
            frame: frame.to_string(),
            message: message.into(),
        }
    }

    pub(crate) fn decode(message: impl Into<String>) -> Self {
        Error::Decode {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(m) => write!(f, "invalid JSON: {m}"),
            Error::FrameDef {
                frame: None,
                message,
            } => write!(f, "invalid frame definition: {message}"),
            Error::FrameDef {
                frame: Some(name),
                message,
            } => {
                write!(f, "invalid frame `{name}`: {message}")
            }
            Error::SignalDef {
                frame,
                signal,
                message,
            } => {
                write!(f, "invalid signal `{signal}` in frame `{frame}`: {message}")
            }
            Error::Encode { frame, message } => {
                write!(f, "encode failed for frame `{frame}`: {message}")
            }
            Error::Decode { message } => write!(f, "decode failed: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e.to_string())
    }
}
