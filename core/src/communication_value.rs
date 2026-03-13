use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use crate::communication_types::CommunicationType;
use crate::data_types::DataTypes;
use crate::data_value::DataValue;
use crate::rand_u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationValue {
    id: u32,
    comm_type: CommunicationType,
    sender: u64,
    receiver: u64,
    data: BTreeMap<DataTypes, DataValue>,
}

#[allow(dead_code)]
impl CommunicationValue {
    pub fn new(comm_type: CommunicationType) -> Self {
        Self {
            id: rand_u32(),
            comm_type,
            sender: 0,
            receiver: 0,
            data: BTreeMap::new(),
        }
    }

    pub fn with_id(mut self, p0: u32) -> Self {
        self.id = p0;
        self
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn with_sender(mut self, sender: u64) -> Self {
        self.sender = sender;
        self
    }

    pub fn get_sender(&self) -> u64 {
        self.sender
    }

    pub fn with_receiver(mut self, receiver: u64) -> Self {
        self.receiver = receiver;
        self
    }

    pub fn get_receiver(&self) -> u64 {
        self.receiver
    }

    pub fn get_type(&self) -> CommunicationType {
        self.comm_type.clone()
    }

    pub fn is_type(&self, p0: CommunicationType) -> bool {
        self.comm_type == p0
    }

    pub fn add_data(mut self, data: DataTypes, value: DataValue) -> Self {
        self.data.insert(data, value);
        self
    }

    pub fn get_data(&self, data_type: DataTypes) -> &DataValue {
        self.data.get(&data_type).unwrap_or(&DataValue::Null)
    }

    pub fn get_data_container(&self) -> &BTreeMap<DataTypes, DataValue> {
        &self.data
    }
}

impl CommunicationValue {
    /*
     * Frame format (strict new format):
     * [4 bytes u32 total_length] // number of bytes after this field
     * [1 byte communication_type]
     * [1 byte flags]
     * [optional 4 bytes id]      // if flags bit2 set
     * [optional 6 bytes sender]  // if flags bit0 set
     * [optional 6 bytes receiver]// if flags bit1 set
     * [data container bytes...]
     *
     * Flags:
     * bit0 => has sender
     * bit1 => has receiver
     * bit2 => has id
     */
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();

        let has_sender = self.sender != 0;
        let has_receiver = self.receiver != 0;
        let has_id = self.id != 0;

        let mut flags: u8 = 0;
        if has_sender {
            flags |= 0b0000_0001;
        }
        if has_receiver {
            flags |= 0b0000_0010;
        }
        if has_id {
            flags |= 0b0000_0100;
        }

        payload.push(self.comm_type.as_number());
        payload.push(flags);

        if has_id {
            let _ = payload.write_u32::<BigEndian>(self.id);
        }

        if has_sender {
            let sender_be = self.sender.to_be_bytes();
            payload.extend_from_slice(&sender_be[2..]); // 6 bytes
        }

        if has_receiver {
            let receiver_be = self.receiver.to_be_bytes();
            payload.extend_from_slice(&receiver_be[2..]); // 6 bytes
        }

        let container_value = DataValue::container_from_map(&self.data);
        let container_bytes = container_value.to_bytes();
        payload.extend_from_slice(&container_bytes);

        let mut frame = Vec::with_capacity(4 + payload.len());
        let _ = frame.write_u32::<BigEndian>(payload.len() as u32);
        frame.extend_from_slice(&payload);

        frame
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::new(bytes);

        let total_len = cursor.read_u32::<BigEndian>().ok()? as usize;
        if bytes.len() < 4 + total_len {
            return None;
        }

        let frame_end = 4 + total_len;

        let comm_type_num = cursor.read_u8().ok()?;
        let comm_type = CommunicationType::from_number(comm_type_num);

        let flags = cursor.read_u8().ok()?;
        let has_sender = (flags & 0b0000_0001) != 0;
        let has_receiver = (flags & 0b0000_0010) != 0;
        let has_id = (flags & 0b0000_0100) != 0;

        let id = if has_id {
            cursor.read_u32::<BigEndian>().ok()?
        } else {
            0
        };

        let sender = if has_sender {
            let mut buf = [0u8; 8];
            cursor.read_exact(&mut buf[2..]).ok()?;
            u64::from_be_bytes(buf)
        } else {
            0
        };

        let receiver = if has_receiver {
            let mut buf = [0u8; 8];
            cursor.read_exact(&mut buf[2..]).ok()?;
            u64::from_be_bytes(buf)
        } else {
            0
        };

        let pos = cursor.position() as usize;
        if pos > frame_end {
            return None;
        }

        let data_bytes = &bytes[pos..frame_end];
        let data_value = DataValue::from_bytes(data_bytes)?;
        let data = data_value.as_map()?;

        Some(Self {
            id,
            comm_type,
            sender,
            receiver,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::communication_types::CommunicationType;
    use crate::data_types::DataTypes;
    use crate::data_value::DataValue;

    fn roundtrip(cv: CommunicationValue) -> CommunicationValue {
        let bytes = cv.to_bytes();
        let decoded = CommunicationValue::from_bytes(&bytes).expect("failed to deserialize");
        let bytes2 = decoded.to_bytes();
        assert_eq!(bytes, bytes2);
        decoded
    }

    #[test]
    fn test_flags_and_order_without_optional() {
        let cv = CommunicationValue::new(CommunicationType::ping).with_id(0);
        let bytes = cv.to_bytes();

        // [u32 len][type][flags]...
        assert!(bytes.len() >= 6);
        let mut c = Cursor::new(bytes.as_slice());
        let total_len = c.read_u32::<BigEndian>().expect("read len");
        assert_eq!(total_len as usize + 4, bytes.len());

        let typ = c.read_u8().expect("read type");
        assert_eq!(typ, CommunicationType::ping.as_number());

        let flags = c.read_u8().expect("read flags");
        assert_eq!(flags & 0b0000_0111, 0);
    }

    #[test]
    fn test_flags_and_order_with_all_optional() {
        let cv = CommunicationValue::new(CommunicationType::message_send)
            .with_id(0xAABBCCDD)
            .with_sender(0x0000_1122_3344_5566)
            .with_receiver(0x0000_6677_8899_AABB);

        let bytes = cv.to_bytes();
        let mut c = Cursor::new(bytes.as_slice());

        let total_len = c.read_u32::<BigEndian>().expect("len");
        assert_eq!(total_len as usize + 4, bytes.len());

        let typ = c.read_u8().expect("type");
        assert_eq!(typ, CommunicationType::message_send.as_number());

        let flags = c.read_u8().expect("flags");
        assert_eq!(flags & 0b0000_0111, 0b0000_0111);

        let id = c.read_u32::<BigEndian>().expect("id");
        assert_eq!(id, 0xAABBCCDD);

        let mut sender6 = [0u8; 6];
        c.read_exact(&mut sender6).expect("sender");
        assert_eq!(sender6, [0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);

        let mut receiver6 = [0u8; 6];
        c.read_exact(&mut receiver6).expect("receiver");
        assert_eq!(receiver6, [0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB]);
    }

    #[test]
    fn test_roundtrip_complex() {
        let cv = CommunicationValue::new(CommunicationType::update)
            .with_id(1234)
            .with_sender(111)
            .with_receiver(222)
            .add_data(DataTypes::username, DataValue::Str("alice".to_string()))
            .add_data(DataTypes::user_id, DataValue::Number(42))
            .add_data(DataTypes::enabled, DataValue::BoolTrue)
            .add_data(
                DataTypes::user_ids,
                DataValue::Array(vec![DataValue::Number(1), DataValue::Number(2)]),
            );

        let decoded = roundtrip(cv.clone());

        assert_eq!(decoded.get_id(), 1234);
        assert_eq!(decoded.get_sender(), 111);
        assert_eq!(decoded.get_receiver(), 222);
        assert_eq!(decoded.get_type(), CommunicationType::update);
        assert_eq!(
            decoded.get_data(DataTypes::username),
            &DataValue::Str("alice".to_string())
        );
        assert_eq!(decoded.get_data(DataTypes::user_id), &DataValue::Number(42));
    }

    #[test]
    fn test_corrupted_length_returns_none() {
        let mut bad = vec![0u8; 8];
        // total_length claims more than available
        bad[0..4].copy_from_slice(&(1000u32.to_be_bytes()));
        assert!(CommunicationValue::from_bytes(&bad).is_none());
    }
}
