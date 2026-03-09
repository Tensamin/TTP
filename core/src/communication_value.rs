use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::io::Read;

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
     * Format:
     * [1 byte data type]
     * [1 bit has sender, 1 bit has receiver, 1 bit has id, 5 placeholder]
     * [optional 4 bytes id]
     * [optional 6 bytes sender]
     * [optional 6 bytes receiver]
     * [4 bytes length of data]
     * [data container bytes...]
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

        // Use DataValue's serialization for the data container
        let data_value = DataValue::container_from_map(&self.data);
        let data_buf = data_value.to_bytes();

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

        // Use DataValue's deserialization
        let data_value = DataValue::from_bytes(&data_bytes)?;
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

// ===============================================
// Tests
// ===============================================
#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;
    use crate::communication_types::CommunicationType;
    use crate::data_types::DataTypes;
    use crate::data_value::DataValue;

    fn roundtrip(cv: CommunicationValue) -> CommunicationValue {
        let bytes = cv.to_bytes();

        let comm = CommunicationValue::from_bytes(&bytes).expect("Failed to deserialize");
        let comm_bytes = comm.to_bytes();

        println!(
            "Serialized {} bytes: {:02x?}",
            bytes.len(),
            &bytes[..20.min(bytes.len())]
        );

        println!(
            "Serialized {} bytes: {:02x?}",
            comm_bytes.len(),
            &comm_bytes[..20.min(comm_bytes.len())]
        );

        assert_eq!(bytes, comm_bytes);

        comm
    }

    #[test]
    fn test_empty_message_roundtrip() {
        let cv = CommunicationValue::new(CommunicationType::ping).with_id(0);

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
    fn test_array_strings() {
        let mut cv = CommunicationValue::new(CommunicationType::messages_get);

        cv.data.insert(
            DataTypes::user_states,
            DataValue::Array(vec![
                DataValue::Str("1".to_string()),
                DataValue::Str("2".to_string()),
                DataValue::Str("3".to_string()),
            ]),
        );

        let decoded = roundtrip(cv);

        if let Some(DataValue::Array(arr)) = decoded.data.get(&DataTypes::user_states) {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected Str array");
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

    #[test]
    fn test_max_id() {
        let cv = CommunicationValue::new(CommunicationType::ping).with_id(u32::MAX);
        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_id(), u32::MAX);
    }

    #[test]
    fn test_zero_id_explicit() {
        let cv = CommunicationValue::new(CommunicationType::ping).with_id(0);
        // ID of 0 should not write the ID field (flag bit not set)
        let bytes = cv.to_bytes();
        let flags = bytes[1];
        assert!(flags & 0b0010_0000 == 0);
        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_id(), 0);
    }

    #[test]
    fn test_max_sender_receiver() {
        // Test 48-bit max values (since we truncate to 6 bytes)
        let max_48bit: u64 = 0x0000_FFFF_FFFF_FFFF;
        let cv = CommunicationValue::new(CommunicationType::message)
            .with_sender(max_48bit)
            .with_receiver(max_48bit);
        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_sender(), max_48bit);
        assert_eq!(decoded.get_receiver(), max_48bit);
    }

    #[test]
    fn test_sender_only_no_receiver() {
        let cv = CommunicationValue::new(CommunicationType::message).with_sender(12345);
        let bytes = cv.to_bytes();
        let flags = bytes[1];
        assert!(flags & 0b1000_0000 != 0); // has_sender
        assert!(flags & 0b0100_0000 == 0); // has_receiver = 0
        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_sender(), 12345);
        assert_eq!(decoded.get_receiver(), 0);
    }

    #[test]
    fn test_receiver_only_no_sender() {
        let cv = CommunicationValue::new(CommunicationType::message).with_receiver(54321);
        let bytes = cv.to_bytes();
        let flags = bytes[1];
        assert!(flags & 0b1000_0000 == 0); // has_sender = 0
        assert!(flags & 0b0100_0000 != 0); // has_receiver
        let decoded = roundtrip(cv);
        assert_eq!(decoded.get_sender(), 0);
        assert_eq!(decoded.get_receiver(), 54321);
    }

    #[test]
    fn test_all_flags_set() {
        let cv = CommunicationValue::new(CommunicationType::message_send)
            .with_id(100)
            .with_sender(200)
            .with_receiver(300);
        let bytes = cv.to_bytes();
        let flags = bytes[1];
        assert_eq!(flags & 0b1110_0000, 0b1110_0000); // all three flags set
    }

    #[test]
    fn test_empty_string_value() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        cv.data
            .insert(DataTypes::username, DataValue::Str("".into()));
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::username),
            Some(&DataValue::Str("".into()))
        );
    }

    #[test]
    fn test_large_number() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        cv.data
            .insert(DataTypes::user_id, DataValue::Number(i64::MAX));
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::user_id),
            Some(&DataValue::Number(i64::MAX))
        );
    }

    #[test]
    fn test_negative_number() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        cv.data.insert(DataTypes::user_id, DataValue::Number(-500));
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::user_id),
            Some(&DataValue::Number(-500))
        );
    }

    #[test]
    fn test_min_number() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        cv.data
            .insert(DataTypes::user_id, DataValue::Number(i64::MIN));
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::user_id),
            Some(&DataValue::Number(i64::MIN))
        );
    }

    #[test]
    fn test_null_value() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        cv.data.insert(DataTypes::error_protocol, DataValue::Null);
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::error_protocol),
            Some(&DataValue::Null)
        );
    }

    #[test]
    fn test_empty_array() {
        let mut cv = CommunicationValue::new(CommunicationType::messages_get);
        cv.data
            .insert(DataTypes::user_ids, DataValue::Array(vec![]));
        let decoded = roundtrip(cv);
        if let Some(DataValue::Array(arr)) = decoded.data.get(&DataTypes::user_ids) {
            assert!(arr.is_empty());
        } else {
            panic!("Expected empty array");
        }
    }

    #[test]
    fn test_large_array() {
        let mut cv = CommunicationValue::new(CommunicationType::messages_get);
        let large_arr: Vec<DataValue> = (0..1000).map(DataValue::Number).collect();
        cv.data
            .insert(DataTypes::user_ids, DataValue::Array(large_arr));
        let decoded = roundtrip(cv);
        if let Some(DataValue::Array(arr)) = decoded.data.get(&DataTypes::user_ids) {
            let arr: &Vec<DataValue> = arr;
            assert_eq!(arr.len(), 1000);
            assert_eq!(arr[0], DataValue::Number(0));
            assert_eq!(arr[999], DataValue::Number(999));
        } else {
            panic!("Expected large array");
        }
    }

    #[test]
    fn test_unicode_string() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        cv.data
            .insert(DataTypes::username, DataValue::Str("用户_αβγ_🎉".into()));
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::username),
            Some(&DataValue::Str("用户_αβγ_🎉".into()))
        );
    }

    #[test]
    fn test_deeply_nested_container() {
        let inner_inner = vec![DataValue::Number(42)];
        let inner = vec![(DataTypes::user_ids, DataValue::Array(inner_inner))];
        let outer = vec![(DataTypes::user, DataValue::Container(inner))];

        let mut cv = CommunicationValue::new(CommunicationType::user_connected);
        cv.data
            .insert(DataTypes::settings, DataValue::Container(outer));

        let decoded = roundtrip(cv);
        if let Some(DataValue::Container(outer_container)) = decoded.data.get(&DataTypes::settings)
        {
            assert_eq!(outer_container.len(), 1);
        } else {
            panic!("Expected nested container");
        }
    }

    #[test]
    fn test_multiple_data_types() {
        let mut cv = CommunicationValue::new(CommunicationType::update);
        cv.data.insert(DataTypes::user_id, DataValue::Number(1));
        cv.data
            .insert(DataTypes::username, DataValue::Str("test".into()));
        cv.data.insert(DataTypes::enabled, DataValue::Bool(true));
        cv.data
            .insert(DataTypes::settings, DataValue::Container(vec![]));
        cv.data.insert(
            DataTypes::user_ids,
            DataValue::Array(vec![DataValue::Number(1)]),
        );

        let decoded = roundtrip(cv);
        assert_eq!(decoded.data.len(), 5);
    }

    #[test]
    fn test_data_not_found_returns_null() {
        let cv = CommunicationValue::new(CommunicationType::ping);
        assert_eq!(cv.get_data(DataTypes::username), &DataValue::Null);
    }

    #[test]
    fn test_is_type() {
        let cv = CommunicationValue::new(CommunicationType::ping);
        assert!(cv.is_type(CommunicationType::ping));
        assert!(!cv.is_type(CommunicationType::pong));
    }

    #[test]
    fn test_add_data_chaining() {
        let cv = CommunicationValue::new(CommunicationType::message)
            .add_data(DataTypes::user_id, DataValue::Number(1))
            .add_data(DataTypes::username, DataValue::Str("test".into()));

        assert_eq!(cv.data.len(), 2);
    }

    #[test]
    fn test_corrupted_data_length() {
        // Data length says more bytes than available
        let corrupted = vec![
            0x30, // ping
            0x00, // no flags
            0x00, 0x00, 0x00, 0xFF, // claims 255 bytes of data
        ];
        assert!(CommunicationValue::from_bytes(&corrupted).is_none());
    }

    #[test]
    fn test_corrupted_truncated_header() {
        assert!(CommunicationValue::from_bytes(&[]).is_none());
        assert!(CommunicationValue::from_bytes(&[0x30]).is_none()); // only type, no flags
    }

    #[test]
    fn test_corrupted_invalid_utf8() {
        let corrupted = vec![
            0x16, // message type
            0x00, // no flags
            0x00, 0x00, 0x00, 0x05, // 5 bytes of data
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // garbage data
        ];
        assert!(CommunicationValue::from_bytes(&corrupted).is_none());
    }

    #[test]
    fn test_all_communication_types_roundtrip() {
        for comm_type in CommunicationType::iter() {
            let cv = CommunicationValue::new(comm_type.clone());
            let bytes = cv.to_bytes();
            let decoded = CommunicationValue::from_bytes(&bytes)
                .expect(&format!("Failed for {:?}", comm_type));
            assert_eq!(decoded.get_type(), comm_type);
        }
    }

    #[test]
    fn test_large_data_payload() {
        let mut cv = CommunicationValue::new(CommunicationType::message);
        let large_string = "x".repeat(10000);
        cv.data
            .insert(DataTypes::content, DataValue::Str(large_string.clone()));
        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::content),
            Some(&DataValue::Str(large_string))
        );
    }

    #[test]
    fn test_bool_true_false_variants() {
        let mut cv = CommunicationValue::new(CommunicationType::update);
        cv.data.insert(DataTypes::enabled, DataValue::BoolTrue);
        cv.data.insert(DataTypes::accepted, DataValue::BoolFalse);

        let decoded = roundtrip(cv);
        assert_eq!(
            decoded.data.get(&DataTypes::enabled),
            Some(&DataValue::BoolTrue)
        );
        assert_eq!(
            decoded.data.get(&DataTypes::accepted),
            Some(&DataValue::BoolFalse)
        );
    }

    #[test]
    fn test_container_preserves_order() {
        let mut cv = CommunicationValue::new(CommunicationType::user_connected);
        cv.data.insert(DataTypes::user_id, DataValue::Number(1));
        cv.data
            .insert(DataTypes::username, DataValue::Str("a".into()));
        cv.data
            .insert(DataTypes::avatar, DataValue::Str("b".into()));

        let bytes1 = cv.to_bytes();
        let decoded = CommunicationValue::from_bytes(&bytes1).unwrap();
        let bytes2 = decoded.to_bytes();

        assert_eq!(bytes1, bytes2);
    }
}
