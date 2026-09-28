use std::collections::HashSet;

use crate::compiler::{CompiledFrame, Signal};
use crate::error::CodecError;

pub(crate) struct ActiveSet {
    names: HashSet<String>,
}

impl ActiveSet {
    pub(crate) fn from_encoded_values(
        frame: &CompiledFrame,
        values: &std::collections::BTreeMap<String, f64>,
    ) -> Result<Self, CodecError> {
        let selector_index = frame.selector_index.ok_or_else(|| {
            CodecError::Invalid(format!(
                "frame 0x{:X} has no selector; internal selector lookup failed",
                frame.id
            ))
        })?;
        let selector_name = &frame.signals[selector_index].name;
        let selector_physical = *values.get(selector_name).ok_or_else(|| {
            CodecError::Invalid(format!("missing required signal '{selector_name}'"))
        })?;
        let selector_raw =
            crate::value::physical_to_raw(&frame.signals[selector_index], selector_physical)?;
        Ok(Self::with_selector(frame, Some(selector_raw)))
    }

    pub(crate) fn with_selector(frame: &CompiledFrame, selector_raw: Option<u32>) -> Self {
        let names = frame
            .signals
            .iter()
            .filter(|signal| signal_is_active(signal, selector_raw))
            .map(|signal| signal.name.clone())
            .collect();
        Self { names }
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    pub(crate) fn missing(
        &self,
        supplied: &std::collections::BTreeMap<String, f64>,
    ) -> Vec<String> {
        self.names
            .iter()
            .filter(|name| !supplied.contains_key(name.as_str()))
            .cloned()
            .collect()
    }
}

fn signal_is_active(signal: &Signal, selector_raw: Option<u32>) -> bool {
    match (&signal.condition, selector_raw) {
        (None, _) => true,
        (Some((_, value)), Some(selected)) => *value == selected,
        (Some(_), None) => false,
    }
}
