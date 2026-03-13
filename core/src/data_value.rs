use base64::Engine;
use base64::engine::general_purpose;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::io::Cursor;

use crate::data_types::DataTypes;

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
    const KIND_BOOL_TRUE: u8 = 0x01;
    const KIND_BOOL_FALSE: u8 = 0x02;
    const KIND_NUMBER: u8 = 0x03;
    const KIND_STR: u8 = 0x04;
    const KIND_ARRAY: u8 = 0x05;
    const KIND_CONTAINER: u8 = 0x06;
    const KIND_NULL: u8 = 0x07;

    pub fn container_from_map(map: &BTreeMap<DataTypes, DataValue>) -> DataValue {
        let mut container = Vec::new();
        for (key, value) in map {
            container.push((key.clone(), value.clone()));
        }
        DataValue::Container(container)
    }

    pub fn kind(&self) -> DataKind {
        match self {
            DataValue::Bool(_) | DataValue::BoolTrue | DataValue::BoolFalse => DataKind::Bool,
            DataValue::Number(_) => DataKind::Number,
            DataValue::Str(_) => DataKind::Str,
            DataValue::Array(a) => {
                if let Some(first) = a.first() {
                    DataKind::Array(Box::new(first.kind()))
                } else {
                    DataKind::Array(Box::new(DataKind::Null))
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
            DataValue::Bool(v) => Some(*v),
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
        self.as_str().map(|s| s.to_string())
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
                let mut out = BTreeMap::new();
                for (k, v) in c {
                    out.insert(k.clone(), v.clone());
                }
                Some(out)
            }
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            DataValue::Container(entries) => Self::encode_container(entries),
            DataValue::Array(arr) => Self::encode_array(arr),
            _ => {
                let mut out = Vec::new();
                if Self::write_value_payload(&mut out, self).is_none() {
                    return Vec::new();
                }
                out
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::new(bytes);
        let value = Self::read_value(&mut cursor, true)?;
        if cursor.position() as usize != bytes.len() {
            return None;
        }
        Some(value)
    }

    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(self.to_bytes())
    }

    pub fn from_base64(base64_str: &str) -> Option<Self> {
        let bytes = general_purpose::STANDARD.decode(base64_str).ok()?;
        Self::from_bytes(&bytes)
    }

    fn encode_container(entries: &[(DataTypes, DataValue)]) -> Vec<u8> {
        let mut out = Vec::new();
        if out
            .write_u16::<BigEndian>(u16::try_from(entries.len()).ok().unwrap_or(0))
            .is_err()
        {
            return Vec::new();
        }

        for (key, value) in entries {
            if !Self::write_container_entry(&mut out, key.clone(), value) {
                return Vec::new();
            }
        }
        out
    }

    fn write_container_entry(buf: &mut Vec<u8>, key: DataTypes, value: &DataValue) -> bool {
        let kind = Self::kind_marker(value);
        buf.push(kind);

        if kind == Self::KIND_BOOL_TRUE || kind == Self::KIND_BOOL_FALSE {
            buf.push(key.as_number());
            return true;
        }

        let mut payload = Vec::new();
        if Self::write_value_payload(&mut payload, value).is_none() {
            return false;
        }

        let len_u16 = match u16::try_from(payload.len()) {
            Ok(v) => v,
            Err(_) => return false,
        };

        if buf.write_u16::<BigEndian>(len_u16).is_err() {
            return false;
        }
        buf.push(key.as_number());
        buf.extend_from_slice(&payload);
        true
    }

    fn encode_array(arr: &[DataValue]) -> Vec<u8> {
        let mut out = Vec::new();
        if out
            .write_u16::<BigEndian>(u16::try_from(arr.len()).ok().unwrap_or(0))
            .is_err()
        {
            return Vec::new();
        }

        for value in arr {
            if !Self::write_array_entry(&mut out, value) {
                return Vec::new();
            }
        }

        out
    }

    fn write_array_entry(buf: &mut Vec<u8>, value: &DataValue) -> bool {
        let kind = Self::kind_marker(value);
        buf.push(kind);

        if kind == Self::KIND_BOOL_TRUE || kind == Self::KIND_BOOL_FALSE {
            return true;
        }

        let mut payload = Vec::new();
        if Self::write_value_payload(&mut payload, value).is_none() {
            return false;
        }

        let len_u16 = match u16::try_from(payload.len()) {
            Ok(v) => v,
            Err(_) => return false,
        };

        if buf.write_u16::<BigEndian>(len_u16).is_err() {
            return false;
        }
        buf.extend_from_slice(&payload);
        true
    }

    fn write_value_payload(buf: &mut Vec<u8>, value: &DataValue) -> Option<()> {
        match value {
            DataValue::BoolTrue => Some(()),
            DataValue::BoolFalse => Some(()),
            DataValue::Bool(v) => {
                if *v {
                    Some(())
                } else {
                    Some(())
                }
            }
            DataValue::Number(n) => {
                buf.write_i64::<BigEndian>(*n).ok()?;
                Some(())
            }
            DataValue::Str(s) => {
                buf.extend_from_slice(s.as_bytes());
                Some(())
            }
            DataValue::Array(arr) => {
                let bytes = Self::encode_array(arr);
                buf.extend_from_slice(&bytes);
                Some(())
            }
            DataValue::Container(entries) => {
                let bytes = Self::encode_container(entries);
                buf.extend_from_slice(&bytes);
                Some(())
            }
            DataValue::Null => Some(()),
        }
    }

    fn read_value(cursor: &mut Cursor<&[u8]>, top_level: bool) -> Option<Self> {
        if top_level {
            let start = cursor.position() as usize;
            let remaining = cursor.get_ref().len().checked_sub(start)?;
            if remaining < 2 {
                return None;
            }

            let snapshot = cursor.clone();
            if let Some(container) = Self::try_read_container(cursor) {
                return Some(container);
            }
            *cursor = snapshot;

            let array = Self::read_array(cursor)?;
            return Some(array);
        }

        let kind = cursor.read_u8().ok()?;
        Self::read_value_by_kind(cursor, kind, None)
    }

    fn try_read_container(cursor: &mut Cursor<&[u8]>) -> Option<Self> {
        let count = cursor.read_u16::<BigEndian>().ok()? as usize;
        let mut entries = Vec::with_capacity(count);

        for _ in 0..count {
            let kind = cursor.read_u8().ok()?;

            if kind == Self::KIND_BOOL_TRUE || kind == Self::KIND_BOOL_FALSE {
                let key = DataTypes::from_number(cursor.read_u8().ok()?);
                let value = if kind == Self::KIND_BOOL_TRUE {
                    DataValue::BoolTrue
                } else {
                    DataValue::BoolFalse
                };
                entries.push((key, value));
                continue;
            }

            let len = cursor.read_u16::<BigEndian>().ok()? as usize;
            let key = DataTypes::from_number(cursor.read_u8().ok()?);

            let start = cursor.position() as usize;
            let end = start.checked_add(len)?;
            if end > cursor.get_ref().len() {
                return None;
            }

            let payload = &cursor.get_ref()[start..end];
            let mut inner = Cursor::new(payload);
            let value = Self::read_value_by_kind(&mut inner, kind, Some(len))?;
            if inner.position() as usize != len {
                return None;
            }

            cursor.set_position(end as u64);
            entries.push((key, value));
        }

        Some(DataValue::Container(entries))
    }

    fn read_array(cursor: &mut Cursor<&[u8]>) -> Option<Self> {
        let count = cursor.read_u16::<BigEndian>().ok()? as usize;
        let mut out = Vec::with_capacity(count);

        for _ in 0..count {
            let kind = cursor.read_u8().ok()?;

            if kind == Self::KIND_BOOL_TRUE {
                out.push(DataValue::BoolTrue);
                continue;
            }
            if kind == Self::KIND_BOOL_FALSE {
                out.push(DataValue::BoolFalse);
                continue;
            }

            let len = cursor.read_u16::<BigEndian>().ok()? as usize;
            let start = cursor.position() as usize;
            let end = start.checked_add(len)?;
            if end > cursor.get_ref().len() {
                return None;
            }

            let payload = &cursor.get_ref()[start..end];
            let mut inner = Cursor::new(payload);
            let value = Self::read_value_by_kind(&mut inner, kind, Some(len))?;
            if inner.position() as usize != len {
                return None;
            }

            cursor.set_position(end as u64);
            out.push(value);
        }

        Some(DataValue::Array(out))
    }

    fn read_value_by_kind(
        cursor: &mut Cursor<&[u8]>,
        kind: u8,
        payload_len: Option<usize>,
    ) -> Option<Self> {
        match kind {
            Self::KIND_BOOL_TRUE => Some(DataValue::BoolTrue),
            Self::KIND_BOOL_FALSE => Some(DataValue::BoolFalse),
            Self::KIND_NUMBER => Some(DataValue::Number(cursor.read_i64::<BigEndian>().ok()?)),
            Self::KIND_STR => {
                let len = payload_len?;
                let start = cursor.position() as usize;
                let end = start.checked_add(len)?;
                if end > cursor.get_ref().len() {
                    return None;
                }
                let s = std::str::from_utf8(&cursor.get_ref()[start..end])
                    .ok()?
                    .to_string();
                cursor.set_position(end as u64);
                Some(DataValue::Str(s))
            }
            Self::KIND_ARRAY => {
                let start = cursor.position() as usize;
                let len = payload_len?;
                let end = start.checked_add(len)?;
                if end > cursor.get_ref().len() {
                    return None;
                }
                let mut inner = Cursor::new(&cursor.get_ref()[start..end]);
                let arr = Self::read_array(&mut inner)?;
                if inner.position() as usize != len {
                    return None;
                }
                cursor.set_position(end as u64);
                Some(arr)
            }
            Self::KIND_CONTAINER => {
                let start = cursor.position() as usize;
                let len = payload_len?;
                let end = start.checked_add(len)?;
                if end > cursor.get_ref().len() {
                    return None;
                }
                let mut inner = Cursor::new(&cursor.get_ref()[start..end]);
                let c = Self::try_read_container(&mut inner)?;
                if inner.position() as usize != len {
                    return None;
                }
                cursor.set_position(end as u64);
                Some(c)
            }
            Self::KIND_NULL => Some(DataValue::Null),
            _ => None,
        }
    }

    fn kind_marker(value: &DataValue) -> u8 {
        match value {
            DataValue::BoolTrue => Self::KIND_BOOL_TRUE,
            DataValue::BoolFalse => Self::KIND_BOOL_FALSE,
            DataValue::Bool(v) => {
                if *v {
                    Self::KIND_BOOL_TRUE
                } else {
                    Self::KIND_BOOL_FALSE
                }
            }
            DataValue::Number(_) => Self::KIND_NUMBER,
            DataValue::Str(_) => Self::KIND_STR,
            DataValue::Array(_) => Self::KIND_ARRAY,
            DataValue::Container(_) => Self::KIND_CONTAINER,
            DataValue::Null => Self::KIND_NULL,
        }
    }
}

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
