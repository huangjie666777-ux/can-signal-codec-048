use can_signal_codec::{Database, Error};

const DEFINITIONS: &str = include_str!("../definitions/example_frames.json");

fn db() -> Database {
    Database::from_json(DEFINITIONS).expect("definitions must load")
}

#[test]
fn loads_database() {
    let db = db();
    assert!(db.frame_by_name("device_status").is_some());
    assert!(db.frame_by_id(0x1A4).is_some());
    assert!(db.frame_by_name("ext_sensor").is_some());
}

#[test]
fn encodes_signed_branch_with_scaling() {
    let db = db();
    // raw temp = (23.5 - (-40)) / 0.5 = 127 (7-bit-fit signed 10-bit).
    let frame = db
        .encode(
            "device_status",
            &[("mode", 1.0), ("counter", 7.0), ("temp_c", 23.5)],
        )
        .unwrap();
    assert_eq!(frame.id, 0x1A4);
    assert_eq!(frame.data, vec![0x01, 0x07, 0x7F, 0x00]);

    let decoded = db.decode(frame.id, false, &frame.data).unwrap();
    assert_eq!(decoded["mode"].raw, 1);
    assert_eq!(decoded["counter"].raw, 7);
    assert_eq!(decoded["temp_c"].raw, 127);
    assert!((decoded["temp_c"].physical - 23.5).abs() < 1e-9);
    assert!(!decoded.contains_key("voltage"));
}

#[test]
fn motorola_branch_reuses_positions() {
    let db = db();
    let frame = db
        .encode(
            "device_status",
            &[("mode", 2.0), ("counter", 9.0), ("voltage", 12.34)],
        )
        .unwrap();
    // raw voltage = 12.34 / 0.01 = 1234 = 0x4D2, laid out Motorola from bit 31.
    assert_eq!(frame.data, vec![0x02, 0x09, 0x9A, 0x40]);
    let decoded = db.decode(frame.id, false, &frame.data).unwrap();
    assert!((decoded["voltage"].physical - 12.34).abs() < 1e-9);
    assert!(!decoded.contains_key("temp_c"));
}

#[test]
fn no_matching_branch_keeps_only_selector_and_residents() {
    let db = db();
    let frame = db
        .encode("device_status", &[("mode", 3.0), ("counter", 1.0)])
        .unwrap();
    let decoded = db.decode(frame.id, false, &frame.data).unwrap();
    assert!(decoded.contains_key("mode"));
    assert!(decoded.contains_key("counter"));
    assert!(!decoded.contains_key("temp_c"));
    assert!(!decoded.contains_key("voltage"));
}

#[test]
fn rejects_missing_unknown_and_inactive_fields() {
    let db = db();
    let err = db
        .encode("device_status", &[("mode", 1.0), ("counter", 7.0)])
        .unwrap_err();
    assert!(matches!(err, Error::Encode { .. }), "{err}");
    assert!(err.to_string().contains("missing active signal `temp_c`"));

    let err = db
        .encode(
            "device_status",
            &[
                ("mode", 1.0),
                ("counter", 7.0),
                ("temp_c", 23.5),
                ("voltage", 5.0),
            ],
        )
        .unwrap_err();
    assert!(err.to_string().contains("inactive"));

    let err = db
        .encode(
            "device_status",
            &[
                ("mode", 1.0),
                ("counter", 7.0),
                ("nope", 1.0),
                ("temp_c", 23.5),
            ],
        )
        .unwrap_err();
    assert!(err.to_string().contains("unknown signal `nope`"));
}

#[test]
fn rejects_overflow_and_non_finite_encoding() {
    let db = db();
    let err = db
        .encode(
            "device_status",
            &[("mode", 1.0), ("counter", 7.0), ("temp_c", 500.0)],
        )
        .unwrap_err();
    assert!(err.to_string().contains("outside range"));
    let err = db
        .encode(
            "device_status",
            &[("mode", 1.0), ("counter", 7.0), ("temp_c", f64::NAN)],
        )
        .unwrap_err();
    assert!(err.to_string().contains("not finite"));
}

#[test]
fn decodes_two_complement_negative() {
    let db = db();
    // raw -1 as 10-bit two's complement = 0x3FF at bits 16..25 (Intel).
    let data = [0x01u8, 0x00, 0xFF, 0x03];
    let decoded = db.decode(0x1A4, false, &data).unwrap();
    assert_eq!(decoded["temp_c"].raw, -1);
    assert!((decoded["temp_c"].physical - (-40.5)).abs() < 1e-9);
}

#[test]
fn decode_checks_identity_and_length() {
    let db = db();
    assert!(matches!(
        db.decode(0x9999, false, &[0; 4]).unwrap_err(),
        Error::Decode { .. }
    ));
    let err = db.decode(0x1A4, true, &[0; 4]).unwrap_err();
    assert!(err.to_string().contains("identity mismatch"));
    let err = db.decode(0x1A4, false, &[0; 3]).unwrap_err();
    assert!(err.to_string().contains("payload bytes"));
}

#[test]
fn rejects_bad_definitions() {
    let must_fail = |json: &str, needle: &str| {
        let err = Database::from_json(json).unwrap_err();
        assert!(err.to_string().contains(needle), "error was: {err}");
    };

    must_fail(
        r#"{"frames":[{"name":"f","id":2048,"length":2,"signals":[
           {"name":"s","start_bit":0,"width":1,"endian":"intel"}]}]}"#,
        "out of range",
    );
    must_fail(
        r#"{"frames":[{"name":"f","id":7,"length":9,"signals":[
           {"name":"s","start_bit":0,"width":1,"endian":"intel"}]}]}"#,
        "1..=8",
    );
    must_fail(
        r#"{"frames":[{"name":"f","id":7,"length":2,"signals":[
           {"name":"s","start_bit":15,"width":2,"endian":"intel"}]}]}"#,
        "exceed",
    );
    must_fail(
        r#"{"frames":[{"name":"f","id":7,"length":2,"signals":[
           {"name":"s","start_bit":0,"width":1,"endian":"intel","factor":0}]}]}"#,
        "non-zero",
    );
    must_fail(
        r#"{"frames":[{"name":"f","id":7,"length":2,"signals":[
           {"name":"a","start_bit":0,"width":4,"endian":"intel"},
           {"name":"b","start_bit":2,"width":4,"endian":"intel"}]}]}"#,
        "overlaps",
    );
    must_fail(
        r#"{"frames":[{"name":"f","id":7,"length":2,"signals":[
           {"name":"m","start_bit":0,"width":2,"endian":"intel","selector":true,"signed":true}]}]}"#,
        "selector must be unsigned",
    );
    must_fail(
        r#"{"frames":[{"name":"f","id":7,"length":2,"signals":[
           {"name":"m","start_bit":0,"width":2,"endian":"intel","selector":true},
           {"name":"x","start_bit":8,"width":1,"endian":"intel","when":7}]}]}"#,
        "outside selector range",
    );
}

#[test]
fn rounded_nearest_half_away_from_zero() {
    let db = db();
    // 23.6 -> raw (23.6+40)/0.5 = 127.2 -> 127; 23.65 -> 127.3 -> 127;
    // use a midpoint: physical -40 + 127.5*0.5 = 23.75 -> raw 127.5 -> 128.
    let frame = db
        .encode(
            "device_status",
            &[("mode", 1.0), ("counter", 0.0), ("temp_c", 23.75)],
        )
        .unwrap();
    assert_eq!(frame.data[2], 0x80);
}
