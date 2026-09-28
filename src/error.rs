use std::fmt;

/// A frame or signal definition could not be compiled.
#[derive(Debug)]
pub enum DefinitionError {
    /// The JSON document did not match the accepted schema.
    Json(serde_json::Error),
    /// The JSON matched the schema but described an invalid CAN definition.
    Invalid(String),
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(source) => write!(f, "invalid frame definition JSON: {source}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for DefinitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Json(error) = self {
            return Some(error);
        }
        None
    }
}

impl From<serde_json::Error> for DefinitionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// A message could not be encoded or decoded.
#[derive(Debug)]
pub enum CodecError {
    /// The supplied physical values or CAN message did not match the frame.
    Invalid(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CodecError {}
