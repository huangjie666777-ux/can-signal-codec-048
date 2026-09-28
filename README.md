# CAN signal codec

一个可嵌入 Rust 应用的 CAN 信号编解码库。它只负责根据 JSON 帧定义读写负载字节，不执行总线收发，也不解析 DBC。

## JSON 格式

顶层是 frames 数组。每个帧定义包含：

- id：CAN 仲裁 ID；标准帧最大 0x7FF，扩展帧最大 0x1FFFFFFF。
- extended：可选，true 表示 29 位扩展 ID；默认 false。
- dlc：负载长度，必须为 1..=8。
- signals：至少一个信号。

信号字段：

- name：帧内唯一、非空名称。
- start_bit：起始位，首字节最低有效位为 0，每字节连续编号到 7，下一字节从 8 开始。
- width：位宽，范围 1..=32。
- byte_order：intel 或 motorola。
- signed：是否为二进制补码有符号整数。
- factor、offset：物理值 = 原始整数 × factor + offset；二者必须有限，factor 不能为零。
- is_selector：可选，标记该帧唯一的无符号选择器；选择器必须 factor 为 1、offset 为 0。
- condition：可选，格式为 selector 加 value，表示信号仅在该选择器原始值下激活；不支持嵌套条件。

Intel 信号的起始位是最低有效位，后续位号递增。Motorola 信号的起始位是最高有效位，后续位号递减；走到一个字节的最低位后，下一位跳到下一字节最高位，即位号加 15。例如从位 31 开始的 4 位 Motorola 信号使用位 31、30、29、24，原始值从 LSB 到 MSB 分别映射到这些线位。

同一位位置只允许被不可能同时激活的互斥分支复用。常驻信号与任何重叠信号冲突；绑定同一个选择器的不同值可以复用位置；没有匹配条件时，只处理选择器和常驻信号。

示例：

    {
      "frames": [
        {
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
        }
      ]
    }

## 公共 API

- Database::from_json(json)：从字符串编译定义。
- Database::from_json_bytes(bytes)：从 JSON 字节编译定义。
- Database::from_json_file(path)：从文件读取并编译定义。
- Database::frame(id, extended)：取得已验证的帧。
- Frame::encode(values) 或 Database::encode_frame(id, extended, values)：物理值编码为 EncodedFrame。
- Frame::decode(message) 或 Database::decode(message)：解码 CanMessage。
- DecodedSignal 包含 raw 和 physical，分别是原始整数位模式和物理值。

编码时必须恰好提供当前选择器值下的全部激活信号：缺失、未知、非激活字段都会失败。编码采用就近舍入，半值远离零；非有限输入、非有限运算结果、整数溢出和超范围原始值都会失败。所有未使用位会被清零。

解码时会核对标准/扩展帧身份和精确负载长度，只返回当前激活的信号。有符号信号的 raw 是 32 位内的二进制补码位模式。

## 使用

    use std::collections::BTreeMap;
    use can_signal_codec::{CanMessage, Database};

    let db = Database::from_json(DEFINITION_JSON)?;
    let mut values = BTreeMap::new();
    values.insert("mode".to_string(), 1.0);
    values.insert("rpm".to_string(), 100.0);
    values.insert("temperature".to_string(), -44.0);

    let encoded = db.encode_frame(0x102, false, &values)?;
    let decoded = db.decode(&CanMessage {
        id: encoded.message.id,
        extended: encoded.message.extended,
        data: encoded.data().to_vec(),
    })?;

## 构建、测试和示例

    cargo build --locked
    cargo test --locked
    cargo run --example print_codec --locked

示例输出的报文字节为：

    encoded bytes: E1 06 00 C0
    mode: raw=1, physical=1
    rpm: raw=440, physical=100
    temperature: raw=12, physical=-44

依赖源码已通过 Cargo source replacement 放在 vendor 目录中。
