//! Compilation and whole-definition validation.
//!
//! All checks run before any compiled object is returned, so callers never
//! observe a half-built database.

use std::collections::{BTreeMap, HashMap};

use crate::bits::{layout_positions, Endian};
use crate::error::{Error, Result};
use crate::schema::{FrameDef, Schema, SignalDef};

/// One compiled signal.
#[derive(Debug, Clone)]
pub struct CompiledSignal {
    pub name: String,
    /// Occupied bit positions, MSB-first (see [`crate::bits`]).
    pub positions: Vec<u16>,
    pub width: u8,
    pub endian: Endian,
    pub signed: bool,
    pub factor: f64,
    pub offset: f64,
    pub is_selector: bool,
    /// `None` for resident signals, `Some(v)` for selector branch members.
    pub when: Option<i64>,
}

impl CompiledSignal {
    /// Inclusive raw range representable by this signal.
    pub fn raw_range(&self) -> (i64, i64) {
        if self.signed {
            (-(1i64 << (self.width - 1)), (1i64 << (self.width - 1)) - 1)
        } else {
            (
                0,
                if self.width == 32 {
                    i64::from(u32::MAX)
                } else {
                    (1i64 << self.width) - 1
                },
            )
        }
    }
}

/// One compiled frame.
#[derive(Debug, Clone)]
pub struct CompiledFrame {
    pub name: String,
    pub id: u32,
    pub extended: bool,
    pub length: u8,
    signals: Vec<CompiledSignal>,
    selector_idx: Option<usize>,
    /// Indices of resident (always active) signals, excluding the selector.
    resident: Vec<usize>,
    /// Selector raw value -> indices of signals in that branch.
    branches: BTreeMap<i64, Vec<usize>>,
}

impl CompiledFrame {
    pub fn signals(&self) -> &[CompiledSignal] {
        &self.signals
    }

    pub(crate) fn selector(&self) -> Option<&CompiledSignal> {
        self.selector_idx.map(|i| &self.signals[i])
    }

    /// Indices of signals active for a given raw selector value.
    ///
    /// For frames without a selector every signal is resident. When no branch
    /// matches, only the selector and resident signals are active.
    pub(crate) fn active_indices(&self, selector_raw: Option<i64>) -> Vec<usize> {
        let mut active = self.resident.clone();
        if let Some(idx) = self.selector_idx {
            active.push(idx);
        }
        if let Some(value) = selector_raw {
            if let Some(members) = self.branches.get(&value) {
                active.extend_from_slice(members);
            }
        }
        active
    }

    /// Signal index lookup by name.
    pub fn signal_index(&self, name: &str) -> Option<usize> {
        self.signals.iter().position(|s| s.name == name)
    }
}

/// A fully validated, immutable collection of frames.
#[derive(Debug, Clone, Default)]
pub struct Database {
    frames: Vec<CompiledFrame>,
    by_name: HashMap<String, usize>,
    by_id: HashMap<u32, usize>,
}

impl Database {
    /// Parse and validate a JSON document.
    pub fn from_json(json: &str) -> Result<Self> {
        let schema: Schema = serde_json::from_str(json)?;
        compile(&schema)
    }

    pub fn frames(&self) -> &[CompiledFrame] {
        &self.frames
    }

    pub fn frame_by_name(&self, name: &str) -> Option<&CompiledFrame> {
        self.by_name.get(name).map(|&i| &self.frames[i])
    }

    pub fn frame_by_id(&self, id: u32) -> Option<&CompiledFrame> {
        self.by_id.get(&id).map(|&i| &self.frames[i])
    }
}

fn compile(schema: &Schema) -> Result<Database> {
    if schema.frames.is_empty() {
        return Err(Error::frame_def(None, "`frames` must not be empty"));
    }

    let mut db = Database::default();
    let mut seen_names: HashMap<String, ()> = HashMap::new();
    let mut seen_ids: HashMap<u32, String> = HashMap::new();

    for frame_def in &schema.frames {
        let frame = compile_frame(frame_def)?;
        if seen_names.contains_key(&frame.name) {
            return Err(Error::frame_def(Some(&frame.name), "duplicate frame name"));
        }
        if let Some(other) = seen_ids.get(&frame.id) {
            return Err(Error::frame_def(
                Some(&frame.name),
                format!("CAN id 0x{:X} already used by frame `{other}`", frame.id),
            ));
        }
        let idx = db.frames.len();
        seen_names.insert(frame.name.clone(), ());
        seen_ids.insert(frame.id, frame.name.clone());
        db.by_name.insert(frame.name.clone(), idx);
        db.by_id.insert(frame.id, idx);
        db.frames.push(frame);
    }
    Ok(db)
}

fn compile_frame(def: &FrameDef) -> Result<CompiledFrame> {
    let f = &def.name;
    if f.is_empty() {
        return Err(Error::frame_def(None, "frame name must not be empty"));
    }
    let max_id = if def.extended { 0x1FFF_FFFF } else { 0x7FF };
    if def.id > max_id {
        return Err(Error::frame_def(
            Some(f),
            format!(
                "id 0x{:X} out of range for {} frame (max 0x{max_id:X})",
                def.id,
                if def.extended { "extended" } else { "standard" }
            ),
        ));
    }
    if !(1..=8).contains(&def.length) {
        return Err(Error::frame_def(
            Some(f),
            format!("payload length {} outside 1..=8 bytes", def.length),
        ));
    }
    if def.signals.is_empty() {
        return Err(Error::frame_def(Some(f), "at least one signal is required"));
    }

    let total_bits = def.length as u16 * 8;
    let mut signals: Vec<CompiledSignal> = Vec::with_capacity(def.signals.len());
    let mut names: HashMap<String, ()> = HashMap::new();

    for sdef in &def.signals {
        let sig = compile_signal(f, sdef, total_bits)?;
        if names.contains_key(&sig.name) {
            return Err(Error::signal_def(f, &sig.name, "duplicate signal name"));
        }
        names.insert(sig.name.clone(), ());
        signals.push(sig);
    }

    // Selector checks.
    let selector_idx = signals.iter().position(|s| s.is_selector);
    if signals.iter().filter(|s| s.is_selector).count() > 1 {
        return Err(Error::frame_def(
            Some(f),
            "at most one selector signal is allowed",
        ));
    }
    if let Some(idx) = selector_idx {
        let sel = &signals[idx];
        if sel.signed {
            return Err(Error::signal_def(f, &sel.name, "selector must be unsigned"));
        }
        if sel.factor != 1.0 || sel.offset != 0.0 {
            return Err(Error::signal_def(
                f,
                &sel.name,
                "selector factor must be 1 and offset 0",
            ));
        }
        if sel.when.is_some() {
            return Err(Error::signal_def(
                f,
                &sel.name,
                "selector cannot itself be conditional",
            ));
        }
    }

    // Condition checks and branch grouping.
    let mut branches: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    let mut resident = Vec::new();
    for (i, sig) in signals.iter().enumerate() {
        match sig.when {
            Some(value) => {
                let sel = selector_idx.ok_or_else(|| {
                    Error::signal_def(
                        f,
                        &sig.name,
                        "`when` requires a selector signal in the same frame",
                    )
                })?;
                let (sel_min, sel_max) = signals[sel].raw_range();
                if value < sel_min || value > sel_max {
                    return Err(Error::signal_def(
                        f,
                        &sig.name,
                        format!(
                            "condition value {value} outside selector range {sel_min}..={sel_max}"
                        ),
                    ));
                }
                branches.entry(value).or_default().push(i);
            }
            None => {
                if !sig.is_selector {
                    resident.push(i);
                }
            }
        }
    }

    // Overlap checks among signals that can be simultaneously active.
    for a in 0..signals.len() {
        for b in (a + 1)..signals.len() {
            let sa = &signals[a];
            let sb = &signals[b];
            let exclusive = match (sa.when, sb.when) {
                (Some(va), Some(vb)) => va != vb, // different branches never coexist
                _ => false,
            };
            if exclusive {
                continue;
            }
            if let Some(bit) = first_overlap(&sa.positions, &sb.positions) {
                return Err(Error::signal_def(
                    f,
                    &sa.name,
                    format!(
                        "bit {bit} overlaps signal `{}` while both can be active",
                        sb.name
                    ),
                ));
            }
        }
    }

    Ok(CompiledFrame {
        name: def.name.clone(),
        id: def.id,
        extended: def.extended,
        length: def.length,
        signals,
        selector_idx,
        resident,
        branches,
    })
}

fn compile_signal(frame: &str, def: &SignalDef, total_bits: u16) -> Result<CompiledSignal> {
    let s = &def.name;
    if s.is_empty() {
        return Err(Error::signal_def(
            frame,
            "<unnamed>",
            "signal name must not be empty",
        ));
    }
    if !(1..=32).contains(&def.width) {
        return Err(Error::signal_def(
            frame,
            s,
            format!("width {} outside 1..=32", def.width),
        ));
    }
    if !def.factor.is_finite() {
        return Err(Error::signal_def(
            frame,
            s,
            "factor must be a finite number",
        ));
    }
    if def.factor == 0.0 {
        return Err(Error::signal_def(frame, s, "factor must be non-zero"));
    }
    if !def.offset.is_finite() {
        return Err(Error::signal_def(
            frame,
            s,
            "offset must be a finite number",
        ));
    }
    let positions = layout_positions(def.endian.into(), def.start_bit, def.width, total_bits)
        .ok_or_else(|| {
            Error::signal_def(
                frame,
                s,
                format!(
                    "bits starting at {} with width {} exceed the {}-byte payload",
                    def.start_bit,
                    def.width,
                    total_bits / 8
                ),
            )
        })?;
    Ok(CompiledSignal {
        name: def.name.clone(),
        positions,
        width: def.width,
        endian: def.endian.into(),
        signed: def.signed,
        factor: def.factor,
        offset: def.offset,
        is_selector: def.selector,
        when: def.when,
    })
}

fn first_overlap(a: &[u16], b: &[u16]) -> Option<u16> {
    // Layouts are short (<=32); linear scan is sufficient.
    a.iter().copied().find(|p| b.contains(p))
}
