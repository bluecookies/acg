use tokio_postgres::types::ToSql;

pub trait SqlValue: ToSql + Send + Sync {
    fn to_sql(&self) -> &(dyn ToSql + Sync);
}

impl<T: ToSql + Send + Sync + Clone> SqlValue for T {
    fn to_sql(&self) -> &(dyn ToSql + Sync) {
        self
    }
}

// TODO: see if I can get rid of this
#[derive(Clone)]
pub enum Value {
    None,
    String(String),
    Bool(bool),
    Integer(i64),
    Float(f64),
}

#[derive(Clone, Copy, serde::Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    String,
    Bool,
    I32,
    I64,
    F32,
    F64,
}

use std::fmt;
impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::String => f.write_str("string"),
            ValueType::Bool => f.write_str("bool"),
            ValueType::I32 => f.write_str("i32"),
            ValueType::I64 => f.write_str("i64"),
            ValueType::F32 => f.write_str("f32"),
            ValueType::F64 => f.write_str("f64"),
        }
    }
}
