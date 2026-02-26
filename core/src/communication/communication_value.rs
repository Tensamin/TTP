use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::io::Cursor;
use std::io::Read;

use crate::communication::communication_types::CommunicationType;
use crate::communication::data_container::DataKind;
use crate::communication::data_container::DataValue;
use crate::communication::data_types::DataTypes;

#[derive(Debug, Clone)]
pub struct CommunicationValue {
    id: u32,
    comm_type: CommunicationType,
    sender: u64,
    receiver: u64,
    data: HashMap<DataTypes, DataValue>,
}

#[allow(dead_code)]
impl CommunicationValue {
    pub fn new(comm_type: CommunicationType) -> Self {
        Self {
            id: 0,
            comm_type,
            sender: 0,
            receiver: 0,
            data: HashMap::new(),
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
}

impl CommunicationValue {
    /*
     * [1 byte data type]
     * [1 bit has sender, 1 bit has receiver, 1 bit has id, 5 placeholder]
     * [optional 4 bytes id]
     * [optional 6 bytes sender]
     * [optional 6 bytes receiver]
     * [4 bytes length of data]
     * [data ...]
     */
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        buf.push(self.comm_type.as_number());

        let mut flags: u8 = 0;
        if self.sender != 0 {
            flags |= 0b1000_0000; // bit 7 = has sender
        }
        if self.receiver != 0 {
            flags |= 0b0100_0000; // bit 6 = has receiver
        }
        if self.id != 0 {
            flags |= 0b0010_0000; // bit 5 = has id
        }

        buf.push(flags);

        if self.id != 0 {
            buf.write_u32::<BigEndian>(self.id).unwrap();
        }

        if self.sender != 0 {
            buf.extend_from_slice(&self.sender.to_be_bytes()[2..]);
        }

        if self.receiver != 0 {
            buf.extend_from_slice(&self.receiver.to_be_bytes()[2..]);
        }

        let mut data_buf = Vec::new();
        Self::write_data_container(&mut data_buf, &self.data);

        buf.write_u32::<BigEndian>(data_buf.len() as u32).unwrap();

        buf.extend_from_slice(&data_buf);

        buf
    }
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::new(bytes);

        let comm_type_num = cursor.read_u8().ok()?;
        let comm_type = CommunicationType::from_number(comm_type_num);

        let flags = cursor.read_u8().ok()?;
        let has_sender = (flags & 0b1000_0000) != 0;
        let has_receiver = (flags & 0b0100_0000) != 0;
        let has_id = (flags & 0b0010_0000) != 0;

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

        let data_len = cursor.read_u32::<BigEndian>().ok()? as usize;

        let mut data_bytes = vec![0u8; data_len];
        cursor.read_exact(&mut data_bytes).ok()?;

        let mut data_cursor = Cursor::new(data_bytes.as_slice());
        let data = Self::read_data_container(&mut data_cursor)?;

        Some(Self {
            id,
            comm_type,
            sender,
            receiver,
            data,
        })
    }
    fn write_array(buf: &mut Vec<u8>, arr: &[DataValue]) {
        buf.write_u16::<BigEndian>(arr.len() as u16).unwrap();

        for value in arr {
            let mut object_bytes = Vec::new();

            match value {
                DataValue::Number(n) => {
                    object_bytes.write_i64::<BigEndian>(*n).unwrap();
                }
                DataValue::Str(s) => {
                    object_bytes.extend_from_slice(s.as_bytes());
                }
                DataValue::Container(inner) => {
                    let mut inner_map = HashMap::new();
                    for (k, v) in inner {
                        inner_map.insert(k.clone(), v.clone());
                    }
                    Self::write_data_container(&mut object_bytes, &inner_map);
                }
                DataValue::Array(inner_arr) => {
                    Self::write_array(&mut object_bytes, inner_arr);
                }
                _ => {}
            }

            buf.write_u16::<BigEndian>(object_bytes.len() as u16)
                .unwrap();

            buf.extend_from_slice(&object_bytes);
        }
    }
    fn write_data_container(buf: &mut Vec<u8>, data: &HashMap<DataTypes, DataValue>) {
        for (key, value) in data {
            buf.push(key.as_number());
            let mut data_bytes = Vec::new();

            match value {
                DataValue::Number(n) => data_bytes.write_i64::<BigEndian>(*n).unwrap(),
                DataValue::Str(s) => data_bytes.extend_from_slice(s.as_bytes()),
                DataValue::Container(inner) => {
                    let mut inner_map = HashMap::new();
                    for (k, v) in inner {
                        inner_map.insert(k.clone(), v.clone());
                    }
                    Self::write_data_container(&mut data_bytes, &inner_map);
                }
                DataValue::Array(arr) => Self::write_array(&mut data_bytes, arr),
                DataValue::BoolTrue => data_bytes.extend_from_slice(&[1 as u8]),
                DataValue::BoolFalse => data_bytes.extend_from_slice(&[0 as u8]),
                DataValue::Bool(b) => {
                    data_bytes.extend_from_slice(&[if *b { 1 as u8 } else { 0 as u8 }]);
                }
                DataValue::Null => {}
            }

            buf.write_u16::<BigEndian>(data_bytes.len() as u16).unwrap(); // 2-byte length
            buf.extend_from_slice(&data_bytes); // N bytes data
        }
    }
    fn read_array(cursor: &mut Cursor<&[u8]>, element_kind: &DataKind) -> Option<Vec<DataValue>> {
        let len = cursor.read_u16::<BigEndian>().ok()? as usize;
        let mut result = Vec::with_capacity(len);

        for _ in 0..len {
            let obj_len = cursor.read_u16::<BigEndian>().ok()? as usize;

            let mut obj_bytes = vec![0u8; obj_len];
            cursor.read_exact(&mut obj_bytes).ok()?;

            let mut inner_cursor = Cursor::new(obj_bytes.as_slice());

            let value = match element_kind {
                DataKind::Number => {
                    let n = inner_cursor.read_i64::<BigEndian>().ok()?;
                    DataValue::Number(n)
                }

                DataKind::Str => {
                    let s = String::from_utf8(obj_bytes).ok()?;
                    DataValue::Str(s)
                }

                DataKind::Container => {
                    let map = Self::read_data_container(&mut inner_cursor)?;
                    DataValue::container_from_map(&map)
                }

                DataKind::Array(inner_kind) => {
                    let arr = Self::read_array(&mut inner_cursor, inner_kind)?;
                    DataValue::Array(arr)
                }

                DataKind::Bool => {
                    let b = inner_cursor.read_u8().ok()? != 0;
                    DataValue::Bool(b)
                }

                DataKind::Null => DataValue::Null,
            };

            result.push(value);
        }

        Some(result)
    }
    fn read_data_container(cursor: &mut Cursor<&[u8]>) -> Option<HashMap<DataTypes, DataValue>> {
        let mut data = HashMap::new();

        while (cursor.position() as usize) < cursor.get_ref().len() {
            let key_num = cursor.read_u8().ok()?;
            let key = DataTypes::from_number(key_num);

            let len = cursor.read_u16::<BigEndian>().ok()? as usize;

            let mut data_bytes = vec![0u8; len];
            cursor.read_exact(&mut data_bytes).ok()?;

            let mut inner_cursor = Cursor::new(data_bytes.as_slice());

            let value = match key.expected_kind() {
                DataKind::Number => {
                    let n = inner_cursor.read_i64::<BigEndian>().ok()?;
                    DataValue::Number(n)
                }

                DataKind::Str => DataValue::Str(String::from_utf8(data_bytes).ok()?),

                DataKind::Container => {
                    let inner_map = Self::read_data_container(&mut inner_cursor)?;
                    DataValue::container_from_map(&inner_map)
                }

                DataKind::Array(kind) => {
                    let array = Self::read_array(&mut inner_cursor, &kind)?;
                    DataValue::Array(array)
                }

                DataKind::Bool => {
                    let b = inner_cursor.read_u8().ok()? != 0;
                    DataValue::Bool(b)
                }

                DataKind::Null => DataValue::Null,
            };

            data.insert(key, value);
        }

        Some(data)
    }
}

// ===============================================
// Tests
// ===============================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::communication::communication_types::CommunicationType;
    use crate::communication::data_container::DataValue;
    use crate::communication::data_types::DataTypes;

    fn roundtrip(cv: CommunicationValue) -> CommunicationValue {
        let bytes = cv.to_bytes();
        CommunicationValue::from_bytes(&bytes).expect("Failed to deserialize")
    }

    #[test]
    fn test_empty_message_roundtrip() {
        let cv = CommunicationValue::new(CommunicationType::ping);

        let decoded = roundtrip(cv.clone());

        assert_eq!(decoded.get_type(), cv.get_type());
        assert_eq!(decoded.get_id(), 0);
        assert_eq!(decoded.get_sender(), 0);
        assert_eq!(decoded.get_receiver(), 0);
        assert!(decoded.data.is_empty());
    }

    #[test]
    fn test_id_flag() {
        let cv = CommunicationValue::new(CommunicationType::pong).with_id(42);

        let bytes = cv.to_bytes();

        // second byte = flags
        let flags = bytes[1];
        assert!(flags & 0b0010_0000 != 0); // has id

        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_id(), 42);
    }

    #[test]
    fn test_sender_receiver_flags() {
        let cv = CommunicationValue::new(CommunicationType::message_send)
            .with_sender(123456)
            .with_receiver(654321);

        let bytes = cv.to_bytes();
        let flags = bytes[1];

        assert!(flags & 0b1000_0000 != 0); // sender
        assert!(flags & 0b0100_0000 != 0); // receiver

        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_sender(), 123456);
        assert_eq!(decoded.get_receiver(), 654321);
    }

    #[test]
    fn test_string_and_number_data() {
        let mut cv = CommunicationValue::new(CommunicationType::message);

        cv.data
            .insert(DataTypes::username, DataValue::Str("alice".into()));
        cv.data.insert(DataTypes::user_id, DataValue::Number(99));

        let decoded = roundtrip(cv.clone());

        assert_eq!(
            decoded.data.get(&DataTypes::username),
            Some(&DataValue::Str("alice".into()))
        );

        assert_eq!(
            decoded.data.get(&DataTypes::user_id),
            Some(&DataValue::Number(99))
        );
    }

    #[test]
    fn test_bool_values() {
        let mut cv = CommunicationValue::new(CommunicationType::update);

        cv.data.insert(DataTypes::enabled, DataValue::Bool(true));
        cv.data.insert(DataTypes::accepted, DataValue::Bool(false));

        let decoded = roundtrip(cv);

        assert_eq!(
            decoded.data.get(&DataTypes::enabled),
            Some(&DataValue::Bool(true))
        );

        assert_eq!(
            decoded.data.get(&DataTypes::accepted),
            Some(&DataValue::Bool(false))
        );
    }

    #[test]
    fn test_array_numbers() {
        let mut cv = CommunicationValue::new(CommunicationType::messages_get);

        cv.data.insert(
            DataTypes::user_ids,
            DataValue::Array(vec![
                DataValue::Number(1),
                DataValue::Number(2),
                DataValue::Number(3),
            ]),
        );

        let decoded = roundtrip(cv);

        if let Some(DataValue::Array(arr)) = decoded.data.get(&DataTypes::user_ids) {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected number array");
        }
    }

    #[test]
    fn test_nested_container() {
        let mut inner = Vec::new();
        inner.push((DataTypes::username, DataValue::Str("bob".into())));
        inner.push((DataTypes::user_id, DataValue::Number(7)));

        let mut cv = CommunicationValue::new(CommunicationType::user_connected);

        cv.data.insert(DataTypes::user, DataValue::Container(inner));

        let decoded = roundtrip(cv);

        if let Some(DataValue::Container(entries)) = decoded.data.get(&DataTypes::user) {
            assert_eq!(entries.len(), 2);
        } else {
            panic!("Expected container");
        }
    }

    #[test]
    fn test_array_of_containers() {
        let mut container1 = Vec::new();
        container1.push((DataTypes::username, DataValue::Str("alice".into())));

        let mut container2 = Vec::new();
        container2.push((DataTypes::username, DataValue::Str("bob".into())));

        let mut cv = CommunicationValue::new(CommunicationType::error_anonymous);

        cv.data.insert(
            DataTypes::matches,
            DataValue::Array(vec![
                DataValue::Container(container1),
                DataValue::Container(container2),
            ]),
        );

        let decoded = roundtrip(cv);

        if let Some(DataValue::Array(arr)) = decoded.data.get(&DataTypes::matches) {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected container array");
        }
    }

    #[test]
    fn test_corrupted_input_returns_none() {
        let corrupted = vec![0, 0, 0, 0];
        assert!(CommunicationValue::from_bytes(&corrupted).is_none());
    }

    #[test]
    fn test_full_complex_roundtrip() {
        let mut cv = CommunicationValue::new(CommunicationType::message_send)
            .with_id(999)
            .with_sender(111)
            .with_receiver(222);

        cv.data
            .insert(DataTypes::username, DataValue::Str("charlie".into()));
        cv.data.insert(DataTypes::user_id, DataValue::Number(42));
        cv.data.insert(DataTypes::enabled, DataValue::Bool(true));

        let decoded = roundtrip(cv.clone());

        assert_eq!(decoded.get_id(), 999);
        assert_eq!(decoded.get_sender(), 111);
        assert_eq!(decoded.get_receiver(), 222);
        assert_eq!(decoded.get_type(), cv.get_type());
        assert_eq!(decoded.data.len(), cv.data.len());
    }
}
