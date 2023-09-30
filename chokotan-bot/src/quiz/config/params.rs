use std::{collections::HashMap, sync::Arc};

use database::{SqlValue, ValueType};

pub type QuizParameters = Arc<[Box<dyn SqlValue>]>;

// TODO: make sure no dupe params when reading config
#[derive(serde::Deserialize)]
pub(super) struct Parameter {
    name: Box<str>,
    kind: ValueType,
    //TODO: only use this when deserializing,
    // store a Box<dyn SqlValue> after validating
    default: Option<serde_yaml::Value>,
}

impl Parameter {
    fn to_value(&self, input: &mut HashMap<String, String>) -> Result<Box<dyn SqlValue>, Error> {
        self.to_value_inner(input).map_err(|e| Error {
            name: self.name.clone(),
            kind: e,
        })
    }

    fn to_value_inner(
        &self,
        input: &mut HashMap<String, String>,
    ) -> Result<Box<dyn SqlValue>, ErrorKind> {
        use database::ValueType::*;
        let value = if let Some(value) = input.remove(self.name.as_ref()) {
            let value: Box<dyn SqlValue> = match self.kind {
                String => Box::new(value),
                Bool => Box::new(value.parse::<bool>()?),
                I32 => Box::new(value.parse::<i32>()?),
                I64 => Box::new(value.parse::<i64>()?),
                F32 => Box::new(value.parse::<f32>()?),
                F64 => Box::new(value.parse::<f64>()?),
            };
            value
        } else {
            // use the default parameter if it exists
            let value = self
                .default
                .clone()
                .ok_or_else(|| ErrorKind::ParamNotProvided)?;
            let value: Box<dyn SqlValue> = match self.kind {
                String => {
                    if let Some(v) = value.as_str() {
                        Box::new(v.to_string())
                    } else {
                        return Err(ErrorKind::ParseConfigErr(self.kind, value));
                    }
                }
                Bool => {
                    if let Some(v) = value.as_bool() {
                        Box::new(v)
                    } else {
                        return Err(ErrorKind::ParseConfigErr(self.kind, value));
                    }
                }
                I32 => {
                    if let Some(v) = value.as_i64() {
                        let v =
                            i32::try_from(v).map_err(|e| ErrorKind::IntConvertErr(self.kind, e))?;
                        Box::new(v)
                    } else {
                        return Err(ErrorKind::ParseConfigErr(self.kind, value));
                    }
                }
                I64 => {
                    if let Some(v) = value.as_i64() {
                        Box::new(v)
                    } else {
                        return Err(ErrorKind::ParseConfigErr(self.kind, value));
                    }
                }
                F32 => {
                    if let Some(v) = value.as_f64() {
                        Box::new(v as f32)
                    } else {
                        return Err(ErrorKind::ParseConfigErr(self.kind, value));
                    }
                }
                F64 => {
                    if let Some(v) = value.as_f64() {
                        Box::new(v)
                    } else {
                        return Err(ErrorKind::ParseConfigErr(self.kind, value));
                    }
                }
            };
            value
        };
        Ok(value)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("error on parameter `{name}`: {kind}")]
pub struct Error {
    pub name: Box<str>,
    pub kind: ErrorKind,
}

#[derive(thiserror::Error, Debug)]
pub enum ErrorKind {
    #[error("missing parameter with no default")]
    ParamNotProvided,
    #[error("failed to parse bool: {0}")]
    ParseBoolErr(#[from] std::str::ParseBoolError),
    #[error("failed to parse int: {0}")]
    ParseIntErr(#[from] std::num::ParseIntError),
    #[error("failed to parse float: {0}")]
    ParseFloatErr(#[from] std::num::ParseFloatError),
    #[error("failed to convert number to type `{0}`: {1}")]
    IntConvertErr(ValueType, std::num::TryFromIntError),
    // TODO: this should be a config error
    #[error("default parameter value of incorrect type, expected `{0}` for {1:?}")]
    ParseConfigErr(ValueType, serde_yaml::Value),
}

// TODO: return what params were used/what defaults were used
impl super::QuizConfig {
    // Parses parameters passed in from command into the correct type and
    // then casts them to values to pass into database query
    pub(crate) fn parse_params(
        &self,
        mut params: HashMap<String, String>,
    ) -> Result<QuizParameters, Error> {
        let params = self
            .params
            .iter()
            .map(|p| p.to_value(&mut params))
            .collect::<Result<Arc<_>, _>>()?;
        Ok(params)
    }
}
