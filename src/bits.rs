use crate::schema::ByteOrder;

pub(crate) const MAX_SIGNAL_WIDTH: u8 = 32;

#[derive(Debug, Clone)]
pub(crate) struct BitLayout {
    /// Wire position for raw bit 0 through the final raw signal bit.
    pub(crate) raw_bit_positions: Vec<u16>,
}

impl BitLayout {
    pub(crate) fn compile(
        start_bit: u8,
        width: u8,
        byte_order: ByteOrder,
        frame_bits: u16,
    ) -> Result<Self, String> {
        let raw_bit_positions = match byte_order {
            ByteOrder::Intel => (0..u16::from(width))
                .map(|offset| u16::from(start_bit) + offset)
                .collect::<Vec<_>>(),
            ByteOrder::Motorola => {
                let mut start_to_msb = Vec::with_capacity(usize::from(width));
                let mut current = u16::from(start_bit);
                for _ in 0..u16::from(width) {
                    start_to_msb.push(current);
                    current = if current % 8 == 0 {
                        current + 15
                    } else {
                        current - 1
                    };
                }
                start_to_msb.reverse();
                start_to_msb
            }
        };

        if raw_bit_positions
            .iter()
            .any(|position| *position >= frame_bits)
        {
            return Err("signal bit is outside the frame payload".to_string());
        }
        Ok(Self { raw_bit_positions })
    }

    pub(crate) fn positions(&self) -> &[u16] {
        &self.raw_bit_positions
    }

    pub(crate) fn write_raw(&self, data: &mut [u8], raw: u32) {
        for (raw_bit, wire_position) in self.raw_bit_positions.iter().enumerate() {
            let byte_index = usize::from(*wire_position / 8);
            let bit_index = *wire_position % 8;
            let bit = (raw >> raw_bit) & 1;
            data[byte_index] &= !(1 << bit_index);
            data[byte_index] |= (bit as u8) << bit_index;
        }
    }

    pub(crate) fn read_raw(&self, data: &[u8]) -> u32 {
        let mut raw = 0u32;
        for (raw_bit, wire_position) in self.raw_bit_positions.iter().enumerate() {
            let byte_index = usize::from(*wire_position / 8);
            let bit_index = *wire_position % 8;
            raw |= u32::from((data[byte_index] >> bit_index) & 1) << raw_bit;
        }
        raw
    }
}
