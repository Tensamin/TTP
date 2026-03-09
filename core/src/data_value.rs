use base64::Engine;
use base64::engine::general_purpose;

use crate::data_types::DataTypes;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataKind {
    Bool,
    Number,
    Str,
    Array(Box<DataKind>),
    Container,
    Null,
}

#[derive(Debug, Clone, Eq)]
pub enum DataValue {
    BoolTrue,
    BoolFalse,
    Bool(bool),
    Number(i64),
    Str(String),
    Array(Vec<DataValue>),
    Container(Vec<(DataTypes, DataValue)>),
    Null,
}

impl DataValue {
    pub fn container_from_map(map: &BTreeMap<DataTypes, DataValue>) -> DataValue {
        let mut container = Vec::new();
        for (key, value) in map {
            container.push((key.clone(), value.clone()));
        }
        DataValue::Container(container)
    }

    pub fn kind(&self) -> DataKind {
        match self {
            DataValue::Bool(_) => DataKind::Bool,
            DataValue::BoolTrue => DataKind::Bool,
            DataValue::BoolFalse => DataKind::Bool,
            DataValue::Number(_) => DataKind::Number,
            DataValue::Str(_) => DataKind::Str,
            DataValue::Array(a) => {
                if a.is_empty() {
                    DataKind::Array(Box::new(DataKind::Null))
                } else {
                    DataKind::Array(Box::new(a.first().unwrap().kind()))
                }
            }
            DataValue::Container(_) => DataKind::Container,
            DataValue::Null => DataKind::Null,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DataValue::BoolTrue => Some(true),
            DataValue::BoolFalse => Some(false),
            DataValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            DataValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self {
            DataValue::Str(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            DataValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<Vec<DataValue>> {
        match self {
            DataValue::Array(a) => Some(a.clone()),
            _ => None,
        }
    }

    pub fn as_container(&self) -> Option<Vec<(DataTypes, DataValue)>> {
        match self {
            DataValue::Container(c) => Some(c.clone()),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<BTreeMap<DataTypes, DataValue>> {
        match self {
            DataValue::Container(c) => {
                let mut map = BTreeMap::new();
                for (key, value) in c {
                    map.insert(key.clone(), value.clone());
                }
                Some(map)
            }
            _ => None,
        }
    }

    // ===========================================
    // Serialization to/from bytes
    // ===========================================

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.write_to_buffer(&mut buf);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::new(bytes);
        Self::read_from_cursor(&mut cursor)
    }

    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(self.to_bytes())
    }

    pub fn from_base64(base64_str: &str) -> Option<Self> {
        let bytes = general_purpose::STANDARD.decode(base64_str).ok()?;
        Self::from_bytes(&bytes)
    }

    fn write_to_buffer(&self, buf: &mut Vec<u8>) {
        match self {
            DataValue::Number(n) => {
                buf.push(0x01);
                buf.write_i64::<BigEndian>(*n).unwrap();
            }
            DataValue::Str(s) => {
                buf.push(0x02);
                let bytes = s.as_bytes();
                buf.write_u32::<BigEndian>(bytes.len() as u32).unwrap();
                buf.extend_from_slice(bytes);
            }
            DataValue::Bool(b) => {
                buf.push(0x03);
                buf.push(if *b { 1 } else { 0 });
            }
            DataValue::BoolTrue => {
                buf.push(0x03);
                buf.push(1);
            }
            DataValue::BoolFalse => {
                buf.push(0x03);
                buf.push(0);
            }
            DataValue::Array(arr) => {
                buf.push(0x04); // Type marker for Array
                Self::write_array(buf, arr);
            }
            DataValue::Container(inner) => {
                buf.push(0x05); // Type marker for Container
                let mut inner_map = BTreeMap::new();
                for (k, v) in inner {
                    inner_map.insert(k.clone(), v.clone());
                }
                Self::write_data_container(buf, &inner_map);
            }
            DataValue::Null => {
                buf.push(0x06); // Type marker for Null
            }
        }
    }

    // Internal read method
    fn read_from_cursor(cursor: &mut Cursor<&[u8]>) -> Option<Self> {
        let type_marker = cursor.read_u8().ok()?;

        match type_marker {
            0x01 => {
                // Number
                let n = cursor.read_i64::<BigEndian>().ok()?;
                Some(DataValue::Number(n))
            }
            0x02 => {
                // Str
                let len = cursor.read_u32::<BigEndian>().ok()? as usize;
                let mut bytes = vec![0u8; len];
                cursor.read_exact(&mut bytes).ok()?;
                let s = String::from_utf8(bytes).ok()?;
                Some(DataValue::Str(s))
            }
            0x03 => {
                // Bool
                let b = cursor.read_u8().ok()? != 0;
                Some(DataValue::Bool(b))
            }
            0x04 => {
                // Array - need to determine element kind from first element or use Null
                Self::read_array(cursor, &DataKind::Null)
            }
            0x05 => {
                // Container
                let map = Self::read_data_container(cursor)?;
                Some(DataValue::container_from_map(&map))
            }
            0x06 => Some(DataValue::Null),
            _ => None, // Unknown type
        }
    }

    /*
     * Array format:
     * [1 byte element type marker] (if known, 0x00 if unknown/heterogeneous)
     * [2 bytes amount of elements]
     * [elements...]
     */
    fn write_array(buf: &mut Vec<u8>, arr: &[DataValue]) {
        // Write element kind hint (0x00 for heterogeneous/unknown)
        if let Some(first) = arr.first() {
            buf.push(Self::kind_to_marker(&first.kind()));
        } else {
            buf.push(0x00);
        }

        buf.write_u16::<BigEndian>(arr.len() as u16).unwrap();

        for value in arr {
            value.write_to_buffer(buf);
        }
    }

    fn read_array(cursor: &mut Cursor<&[u8]>, _element_kind: &DataKind) -> Option<Self> {
        let _hint = cursor.read_u8().ok()?; // Element type hint, currently unused
        let len = cursor.read_u16::<BigEndian>().ok()? as usize;
        let mut result = Vec::with_capacity(len);

        for _ in 0..len {
            let value = Self::read_from_cursor(cursor)?;
            result.push(value);
        }

        Some(DataValue::Array(result))
    }

    /*
     * Container format:
     * [2 bytes amount of elements]
     * [
     *   [1 byte key]
     *   [value...]
     * ]
     */
    fn write_data_container(buf: &mut Vec<u8>, data: &BTreeMap<DataTypes, DataValue>) {
        buf.write_u16::<BigEndian>(data.len() as u16).unwrap();

        for (key, value) in data {
            buf.push(key.as_number());

            // For Bool types, write inline without length prefix
            if key.expected_kind() == DataKind::Bool {
                match value {
                    DataValue::Bool(b) => buf.push(if *b { 1 } else { 0 }),
                    DataValue::BoolTrue => buf.push(1),
                    DataValue::BoolFalse => buf.push(0),
                    _ => buf.push(0),
                }
            } else {
                // For other types, write the full value
                let mut value_bytes = Vec::new();
                value.write_to_buffer(&mut value_bytes);

                // Remove type marker for container values to save space
                // (type is implied by DataTypes)
                if !value_bytes.is_empty() {
                    // Skip the type marker byte since DataTypes implies the type
                    buf.write_u32::<BigEndian>((value_bytes.len() - 1) as u32)
                        .unwrap();
                    buf.extend_from_slice(&value_bytes[1..]);
                }
            }
        }
    }

    fn read_data_container(cursor: &mut Cursor<&[u8]>) -> Option<BTreeMap<DataTypes, DataValue>> {
        let count = cursor.read_u16::<BigEndian>().ok()? as usize;
        let mut data = BTreeMap::new();

        for _ in 0..count {
            let key_num = cursor.read_u8().ok()?;
            let key = DataTypes::from_number(key_num);
            let expected_kind = key.expected_kind();

            let value = if expected_kind == DataKind::Bool {
                let b = cursor.read_u8().ok()? != 0;
                DataValue::Bool(b)
            } else {
                let len = cursor.read_u32::<BigEndian>().ok()? as usize;
                let mut value_bytes = vec![0u8; len];
                cursor.read_exact(&mut value_bytes).ok()?;

                // Reconstruct with type marker based on expected kind
                let mut full_bytes = vec![Self::kind_to_marker(&expected_kind)];
                full_bytes.extend_from_slice(&value_bytes);

                let mut inner_cursor = Cursor::new(full_bytes.as_slice());
                Self::read_from_cursor(&mut inner_cursor)?
            };

            data.insert(key, value);
        }

        Some(data)
    }

    fn kind_to_marker(kind: &DataKind) -> u8 {
        match kind {
            DataKind::Number => 0x01,
            DataKind::Str => 0x02,
            DataKind::Bool => 0x03,
            DataKind::Array(_) => 0x04,
            DataKind::Container => 0x05,
            DataKind::Null => 0x06,
        }
    }
}

// ===========================================
// Implementation of Partial EQ & Hash
// ===========================================

impl PartialEq for DataValue {
    fn eq(&self, other: &Self) -> bool {
        use DataValue::*;

        match (self, other) {
            (BoolTrue, BoolTrue) | (BoolFalse, BoolFalse) => true,
            (BoolTrue, Bool(true)) | (Bool(true), BoolTrue) => true,
            (BoolFalse, Bool(false)) | (Bool(false), BoolFalse) => true,
            (Bool(a), Bool(b)) => a == b,
            (Number(a), Number(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Array(a), Array(b)) => a == b,
            (Container(a), Container(b)) => a == b,
            (Null, Null) => true,
            _ => false,
        }
    }
}

impl Hash for DataValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use DataValue::*;

        match self {
            BoolTrue | Bool(true) => {
                0u8.hash(state);
                true.hash(state);
            }
            BoolFalse | Bool(false) => {
                0u8.hash(state);
                false.hash(state);
            }
            Number(n) => {
                1u8.hash(state);
                n.hash(state);
            }
            Str(s) => {
                2u8.hash(state);
                s.hash(state);
            }
            Array(a) => {
                3u8.hash(state);
                a.hash(state);
            }
            Container(c) => {
                4u8.hash(state);
                c.hash(state);
            }
            Null => {
                5u8.hash(state);
            }
        }
    }
}

// ===========================================
// Tests
// ===========================================

#[cfg(test)]
#[allow(unused, dead_code)]
mod tests {
    use super::*;
    use crate::data_types::DataTypes;
    use std::collections::BTreeMap;

    fn sample_key_bool() -> DataTypes {
        DataTypes::accepted
    }

    fn sample_key_num() -> DataTypes {
        DataTypes::user_id
    }

    fn sample_key_str() -> DataTypes {
        DataTypes::username
    }

    fn sample_key_container() -> DataTypes {
        DataTypes::settings
    }

    fn sample_key_num_array() -> DataTypes {
        DataTypes::user_ids
    }

    #[test]
    fn test_kind_basic_types() {
        assert_eq!(DataValue::BoolTrue.kind(), DataKind::Bool);
        assert_eq!(DataValue::BoolFalse.kind(), DataKind::Bool);
        assert_eq!(DataValue::Bool(true).kind(), DataKind::Bool);
        assert_eq!(DataValue::Number(42).kind(), DataKind::Number);
        assert_eq!(DataValue::Str("hello".into()).kind(), DataKind::Str);
        assert_eq!(DataValue::Null.kind(), DataKind::Null);
    }

    #[test]
    fn test_kind_array() {
        let arr = DataValue::Array(vec![DataValue::Number(1), DataValue::Number(2)]);
        match arr.kind() {
            DataKind::Array(inner) => {
                assert_eq!(*inner, DataKind::Number);
            }
            _ => panic!("Expected Array kind"),
        }
    }

    #[test]
    fn test_kind_array_empty() {
        let arr = DataValue::Array(vec![]);
        match arr.kind() {
            DataKind::Array(inner) => {
                assert_eq!(*inner, DataKind::Null);
            }
            _ => panic!("Expected Array kind with Null inner"),
        }
    }

    #[test]
    fn test_as_bool() {
        assert_eq!(DataValue::BoolTrue.as_bool(), Some(true));
        assert_eq!(DataValue::BoolFalse.as_bool(), Some(false));
        assert_eq!(DataValue::Bool(true).as_bool(), Some(true));
        assert_eq!(DataValue::Number(1).as_bool(), None);
    }

    #[test]
    fn test_as_str_and_string() {
        let value = DataValue::Str("hello".into());
        assert_eq!(value.as_str(), Some("hello"));
        assert_eq!(value.as_string(), Some("hello".to_string()));
        assert_eq!(DataValue::Number(10).as_str(), None);
    }

    #[test]
    fn test_as_number() {
        let value = DataValue::Number(123);
        assert_eq!(value.as_number(), Some(123));
        assert_eq!(DataValue::BoolTrue.as_number(), None);
    }

    #[test]
    fn test_as_array() {
        let arr = vec![DataValue::Number(1)];
        let value = DataValue::Array(arr.clone());
        assert_eq!(value.as_array(), Some(arr));
        assert_eq!(DataValue::Null.as_array(), None);
    }

    #[test]
    fn test_container_from_map_and_as_map() {
        let mut map = BTreeMap::new();
        map.insert(sample_key_num(), DataValue::Number(1));
        map.insert(sample_key_str(), DataValue::Str("x".into()));

        let container = DataValue::container_from_map(&map);
        let result_map = container.as_map().unwrap();

        assert_eq!(map.len(), result_map.len());
        assert_eq!(
            map.get(&sample_key_str()),
            result_map.get(&sample_key_str())
        );
        assert_eq!(
            map.get(&sample_key_num()),
            result_map.get(&sample_key_num())
        );
    }

    #[test]
    fn test_as_container() {
        let mut map = BTreeMap::new();
        map.insert(sample_key_bool(), DataValue::Number(10));

        let container = DataValue::container_from_map(&map);
        let vec_form = container.as_container().unwrap();

        assert_eq!(vec_form.len(), 1);
        assert_eq!(vec_form[0].1, DataValue::Number(10));
    }

    #[test]
    fn test_equality() {
        assert_eq!(DataValue::Number(5), DataValue::Number(5));
        assert_ne!(DataValue::Number(5), DataValue::Number(6));
        assert_eq!(DataValue::BoolTrue, DataValue::BoolTrue);
        assert_ne!(DataValue::BoolTrue, DataValue::BoolFalse);
        assert_eq!(DataValue::Bool(true), DataValue::BoolTrue);
        assert_ne!(DataValue::Bool(true), DataValue::BoolFalse);
    }

    // New tests for bytes serialization
    #[test]
    fn test_number_roundtrip_bytes() {
        let original = DataValue::Number(42);
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_string_roundtrip_bytes() {
        let original = DataValue::Str("hello world".into());
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_bool_roundtrip_bytes() {
        let original = DataValue::Bool(true);
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);

        let original = DataValue::BoolFalse;
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_array_roundtrip_bytes() {
        let original = DataValue::Array(vec![
            DataValue::Number(1),
            DataValue::Number(2),
            DataValue::Number(3),
        ]);
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_container_roundtrip_bytes() {
        let inner = vec![
            (DataTypes::username, DataValue::Str("alice".into())),
            (DataTypes::user_id, DataValue::Number(42)),
        ];
        let original = DataValue::Container(inner);
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_null_roundtrip_bytes() {
        let original = DataValue::Null;
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = DataValue::Container(vec![
            (DataTypes::username, DataValue::Str("bob".into())),
            (DataTypes::enabled, DataValue::Bool(true)),
        ]);

        let base64_str = original.to_base64();
        let decoded = DataValue::from_base64(&base64_str).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_nested_structure_base64() {
        let inner = DataValue::Container(vec![(DataTypes::user_id, DataValue::Number(123))]);
        let original = DataValue::Container(vec![
            (DataTypes::user, inner),
            (
                DataTypes::user_ids,
                DataValue::Array(vec![DataValue::Number(1), DataValue::Number(2)]),
            ),
        ]);

        let base64_str = original.to_base64();
        let decoded = DataValue::from_base64(&base64_str).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_unicode_base64() {
        let original = DataValue::Str("Hello 世界 🌍".into());
        let base64_str = original.to_base64();
        let decoded = DataValue::from_base64(&base64_str).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_invalid_base64_returns_none() {
        assert!(DataValue::from_base64("not-valid-base64!!!").is_none());
    }

    #[test]
    fn test_empty_array_bytes() {
        let original = DataValue::Array(vec![]);
        let bytes = original.to_bytes();
        let decoded = DataValue::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }
}
