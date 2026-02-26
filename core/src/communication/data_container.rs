use crate::communication::data_types::DataTypes;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataKind {
    Bool,
    Number,
    Str,
    Array(Box<DataKind>),
    Container,
    Null,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
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
    pub fn container_from_map(map: &HashMap<DataTypes, DataValue>) -> DataValue {
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
            DataValue::Array(a) => DataKind::Array(Box::new(a.first().unwrap().kind())),
            DataValue::Container(_) => DataKind::Container,
            DataValue::Null => DataKind::Null,
        }
    }
}
