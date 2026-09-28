use std::collections::BTreeMap;

use can_signal_codec::{CanMessage, Database};

const FRAME_JSON: &str = r#"{
  "frames": [{
    "id": 258,
    "extended": false,
    "dlc": 4,
    "signals": [
      {
        "name": "mode",
        "start_bit": 0,
        "width": 2,
        "byte_order": "intel",
        "signed": false,
        "factor": 1.0,
        "offset": 0.0,
        "is_selector": true
      },
      {
        "name": "rpm",
        "start_bit": 2,
        "width": 10,
        "byte_order": "intel",
        "signed": false,
        "factor": 0.25,
        "offset": -10.0
      },
      {
        "name": "temperature",
        "start_bit": 31,
        "width": 4,
        "byte_order": "motorola",
        "signed": true,
        "factor": 1.0,
        "offset": -40.0,
        "condition": { "selector": "mode", "value": 1 }
      }
    ]
  }]
}"#;

fn values(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), *value))
        .collect()
}

#[test]
fn encodes_and_decodes_intel_motorola_cross_byte_branch() {
    let db = Database::from_json(FRAME_JSON).unwrap();
    let input = values(&[("mode", 1.0), ("rpm", 100.0), ("temperature", -44.0)]);
    let encoded = db.encode_frame(0x102, false, &input).unwrap();

    assert_eq!(encoded.data(), &[0xE1, 0x06, 0x00, 0xC0]);

    let decoded = db
        .decode(&CanMessage {
            id: 0x102,
            extended: false,
            data: encoded.data().to_vec(),
        })
        .unwrap();
    assert_eq!(decoded["mode"].raw, 1);
    assert_eq!(decoded["mode"].physical, 1.0);
    assert_eq!(decoded["rpm"].raw, 440);
    assert_eq!(decoded["rpm"].physical, 100.0);
    assert_eq!(decoded["temperature"].raw, 12);
    assert_eq!(decoded["temperature"].physical, -44.0);
}

#[test]
fn rounds_half_raw_values_away_from_zero() {
    let db = Database::from_json(FRAME_JSON).unwrap();
    let encoded = db
        .encode_frame(0x102, false, &values(&[("mode", 0.0), ("rpm", 100.125)]))
        .unwrap();
    let decoded = db
        .decode(&CanMessage {
            id: 0x102,
            extended: false,
            data: encoded.data().to_vec(),
        })
        .unwrap();
    assert_eq!(decoded["rpm"].raw, 441);
    assert_eq!(decoded["rpm"].physical, 100.25);
}

#[test]
fn inactive_branch_signal_is_forbidden_and_unused_bits_clear() {
    let db = Database::from_json(FRAME_JSON).unwrap();
    let result = db.encode_frame(
        0x102,
        false,
        &values(&[("mode", 0.0), ("rpm", 0.0), ("temperature", 0.0)]),
    );
    assert!(result.is_err());

    let encoded = db
        .encode_frame(0x102, false, &values(&[("mode", 0.0), ("rpm", -10.0)]))
        .unwrap();
    assert_eq!(encoded.data(), &[0, 0, 0, 0]);
}

#[test]
fn rejects_missing_and_unknown_signals() {
    let db = Database::from_json(FRAME_JSON).unwrap();
    assert!(db
        .encode_frame(0x102, false, &values(&[("mode", 1.0), ("rpm", 100.0)]))
        .is_err());
    let mut input = values(&[("mode", 1.0), ("rpm", 100.0), ("temperature", 20.0)]);
    input.insert("unknown".to_string(), 1.0);
    assert!(db.encode_frame(0x102, false, &input).is_err());
}

#[test]
fn validates_frame_identity_and_length() {
    let db = Database::from_json(FRAME_JSON).unwrap();
    assert!(db
        .decode(&CanMessage {
            id: 0x103,
            extended: false,
            data: vec![0, 0, 0, 0],
        })
        .is_err());
    assert!(db
        .decode(&CanMessage {
            id: 0x102,
            extended: false,
            data: vec![0, 0, 0],
        })
        .is_err());
}

#[test]
fn rejects_simultaneously_active_overlap_and_bad_definition() {
    let overlapping = r#"{
      "frames": [{
        "id": 1,
        "dlc": 1,
        "signals": [
          {"name":"resident_a","start_bit":0,"width":2,"byte_order":"intel","signed":false,"factor":1,"offset":0},
          {"name":"resident_b","start_bit":1,"width":2,"byte_order":"intel","signed":false,"factor":1,"offset":0}
        ]
      }]
    }"#;
    assert!(Database::from_json(overlapping).is_err());

    let bad_factor = r#"{
      "frames": [{
        "id": 1,
        "dlc": 1,
        "signals": [
          {"name":"x","start_bit":0,"width":1,"byte_order":"intel","signed":false,"factor":0,"offset":0}
        ]
      }]
    }"#;
    assert!(Database::from_json(bad_factor).is_err());
}

#[test]
fn mutually_exclusive_branch_may_reuse_bits() {
    let json = r#"{
      "frames": [{
        "id": 7,
        "dlc": 1,
        "signals": [
          {"name":"mode","start_bit":0,"width":1,"byte_order":"intel","signed":false,"factor":1,"offset":0,"is_selector":true},
          {"name":"off_value","start_bit":1,"width":2,"byte_order":"intel","signed":false,"factor":1,"offset":0,"condition":{"selector":"mode","value":0}},
          {"name":"on_value","start_bit":1,"width":2,"byte_order":"intel","signed":false,"factor":2,"offset":0,"condition":{"selector":"mode","value":1}}
        ]
      }]
    }"#;
    let db = Database::from_json(json).unwrap();

    let on = db
        .encode_frame(7, false, &values(&[("mode", 1.0), ("on_value", 6.0)]))
        .unwrap();
    assert_eq!(on.data(), &[0x07]);
    let decoded = db
        .decode(&CanMessage {
            id: 7,
            extended: false,
            data: vec![0x07],
        })
        .unwrap();
    assert!(decoded.contains_key("on_value"));
    assert!(!decoded.contains_key("off_value"));
}
