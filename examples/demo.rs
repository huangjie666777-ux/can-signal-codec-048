//! Real encode/decode example. Run with: cargo run --example demo

use can_signal_codec::Database;

const DEFINITIONS: &str = include_str!("../definitions/example_frames.json");

fn main() {
    let db = Database::from_json(DEFINITIONS).expect("valid definitions");

    // Branch 1: selector `mode == 1` activates the signed `temp_c` signal.
    let frame = db
        .encode(
            "device_status",
            &[("mode", 1.0), ("counter", 7.0), ("temp_c", 23.5)],
        )
        .expect("encode");
    println!(
        "encoded 0x{:03X} ({}) bytes: {}",
        frame.id,
        frame.extended,
        frame.data_hex()
    );

    let decoded = db
        .decode(frame.id, frame.extended, &frame.data)
        .expect("decode");
    for name in ["mode", "counter", "temp_c"] {
        let sig = &decoded[name];
        println!("  {name}: raw={}, physical={}", sig.raw, sig.physical);
    }

    // Branch 2: same payload positions reused by `voltage`.
    let frame2 = db
        .encode(
            "device_status",
            &[("mode", 2.0), ("counter", 9.0), ("voltage", 12.34)],
        )
        .expect("encode");
    println!("encoded 0x{:03X} bytes: {}", frame2.id, frame2.data_hex());
    let decoded2 = db
        .decode(frame2.id, frame2.extended, &frame2.data)
        .expect("decode");
    println!(
        "  voltage: physical={} (raw={})",
        decoded2["voltage"].physical, decoded2["voltage"].raw
    );
}
