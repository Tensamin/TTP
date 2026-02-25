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
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Build header byte
        let mut header: u8 = self.comm_type.as_number() & 0b00111111;
        if self.sender != 0 {
            header |= 0b10000000;
        }
        if self.receiver != 0 {
            header |= 0b01000000;
        }
        buf.push(header);

        buf.write_u32::<BigEndian>(self.id).unwrap();

        if self.sender != 0 {
            buf.extend_from_slice(&self.sender.to_be_bytes()[2..]);
        }

        if self.receiver != 0 {
            buf.extend_from_slice(&self.receiver.to_be_bytes()[2..]);
        }

        // Data container
        Self::write_data_container(&mut buf, &self.data);

        buf
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
                DataValue::Bool(b) => {
                    data_bytes.extend_from_slice(&[if *b { 1 as u8 } else { 0 as u8 }]);
                }
                DataValue::Null => {}
            }

            buf.write_u16::<BigEndian>(data_bytes.len() as u16).unwrap(); // 2-byte length
            buf.extend_from_slice(&data_bytes); // N bytes data
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::new(bytes);

        let header = cursor.read_u8().ok()?;
        let has_sender = (header & 0b10000000) != 0;
        let has_receiver = (header & 0b01000000) != 0;
        let comm_type_num = header & 0b00111111;
        let comm_type = CommunicationType::from_number(comm_type_num);

        let id = cursor.read_u32::<BigEndian>().ok()?;

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

        let data = Self::read_data_container(&mut cursor)?;

        Some(Self {
            id,
            comm_type,
            sender,
            receiver,
            data,
        })
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
