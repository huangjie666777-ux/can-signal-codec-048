# CAN Signal Codec

An embeddable Rust library (Rust 1.85.1, `serde`/`serde_json` only) that loads
CAN frame definitions from JSON, then encodes physical values into payload
bytes and decodes received payloads back. It does **not** touch a CAN bus and
does **not** parse DBC files.

## Bit numbering

Bit positions are numbered continuously starting at the least significant bit
of the first byte: byte 0 contains bits 0..=7 (bit 0 = LSB), byte 1 bits
8..=15, and so on.

- `"intel"` (little endian): `start_bit` is the least significant value bit;
  successive value bits occupy `start_bit + 1, +2, ...`. Signals may span
  bytes.
- `"motorola"` (big endian): `start_bit` is the most significant value bit;
  successive bits step down by 1, and after reaching a byte's low bit
  (`position % 8 == 0`) the next step adds 15, landing on the high bit of the
  next byte.

Signed values use two's complement. Widths are 1..=32 bits.

## JSON format

```json
{
  "frames": [
    {
      "name": "device_status",
      "id": 4660,
      "extended": false,
      "length": 4,
      "signals": [
        { "name": "mode",    "start_bit": 0,  "width": 2,  "endian": "intel",
          "signed": false, "factor": 1, "offset": 0, "selector": true },
        { "name": "counter", "start_bit": 8,  "width": 8,  "endian": "intel",
          "signed": false, "factor": 1, "offset": 0 },
        { "name": "temp_c",  "start_bit": 16, "width": 10, "endian": "intel",
          "signed": true, "factor": 0.5, "offset": -40, "when": 1 }
      ]
    }
  ]
}
```

Frame fields:

- `name`: unique frame name.
- `id`: CAN arbitration ID. Max `0x7FF` for standard frames, `0x1FFFFFFF`
  for extended frames.
- `extended`: `true` for a 29-bit ID (default `false`).
- `length`: payload length in bytes, 1..=8.
- `signals`: non-empty list with unique names.

Signal fields:

- `name`, `start_bit`, `width` (1..=32), `endian` (`"intel"`/`"motorola"`,
  also accepts `little`/`big`).
- `signed` (default `false`), `factor` (default `1`, must be finite and
  non-zero), `offset` (default `0`, must be finite).
- `selector: true` marks the frame's single, unsigned multiplex selector
  (`factor` 1, `offset` 0). At most one per frame.
- `when: N` makes the signal active only while the selector's raw value is
  exactly `N`; `N` must fit the selector's raw range. Nesting is not allowed.
- Signals without `when` are resident. Overlapping bits are rejected for any
  pair that can be simultaneously active; signals in different selector
  branches may share the same positions.

Any invalidity rejects the whole document (`Database::from_json` returns an
`Err`); a partially compiled database is never returned.

## Public API

```rust
use can_signal_codec::Database;

let db = Database::from_json(json_text)?;          // compile + validate
let frame = db.encode("device_status", &[          // physical values in
    ("mode", 1.0),
    ("counter", 7.0),
    ("temp_c", 23.5),
])?;
// frame.id, frame.extended, frame.data: Vec<u8>, frame.data_hex()

let signals = db.decode(frame.id, frame.extended, &frame.data)?;
// signals["temp_c"].raw: i64, signals["temp_c"].physical: f64
```

`physical = raw * factor + offset`. Encoding inverts the formula, rounds to
nearest with midpoints rounded away from zero, and rejects overflow and
non-finite results.

Encoding requires **exactly** the active signals — the selector (if any), all
resident signals and the members of the matching branch; missing, unknown,
duplicated and inactive names are errors. Unused bits are emitted as zero.
Decoding verifies the CAN id, standard/extended identity and payload length,
and returns only active signals (when no branch matches, only the selector and
resident signals).

Other exports: `CompiledFrame`, `CompiledSignal`, `CanFrame`,
`DecodedSignal`, `Error`.

## Modules

- `schema` — raw serde JSON structures.
- `compile` — validation and compiled frame/signal representation.
- `bits` — bit numbering, layouts and raw bit I/O.
- `convert` — factor/offset math, rounding and two's-complement conversion.
- `codec` — public `Database::encode` / `Database::decode`.
- `error` — actionable error type.

## Build, test and example

```sh
cargo build
cargo test
cargo run --example demo
```

`cargo run --example demo` prints actual payload bytes and decoded physical
values for both selector branches of the bundled definition in
`definitions/example_frames.json`.
