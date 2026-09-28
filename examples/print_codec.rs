use std::collections::BTreeMap;

use can_signal_codec::{CanMessage, Database};

const DEFINITION: &str = r#"{
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

fn main() {
    let database = Database::from_json(DEFINITION).expect("valid definition");
    let mut values = BTreeMap::new();
    values.insert("mode".to_string(), 1.0);
    values.insert("rpm".to_string(), 100.0);
    values.insert("temperature".to_string(), -44.0);

    let encoded = database
        .encode_frame(0x102, false, &values)
        .expect("values encode");
    let bytes = encoded
        .data()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("encoded bytes: {bytes}");

    let message = CanMessage {
        id: encoded.message.id,
        extended: encoded.message.extended,
        data: encoded.data().to_vec(),
    };
    let decoded = database.decode(&message).expect("message decodes");
    for (name, signal) in &decoded {
        println!("{name}: raw={}, physical={}", signal.raw, signal.physical);
    }
}
