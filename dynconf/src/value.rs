use anyhow::{anyhow, Result};
use std::collections::HashMap;

#[cfg(feature = "json")]
use serde_json;

#[cfg(feature = "yaml")]
use serde_yaml;

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Boolean(bool),
    Integer(i64),
    Array(Vec<Value>),
    Dict(HashMap<String, Value>),
    Null,
}

#[derive(Debug, Clone, Copy)]
pub enum ValueType {
    String,
    Boolean,
    Integer,
    Array,
    Dict,
    Null,
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValueType::String => "string",
                ValueType::Boolean => "boolean",
                ValueType::Integer => "integer",
                ValueType::Array => "array",
                ValueType::Dict => "object",
                ValueType::Null => "null",
            }
        )
    }
}

impl Value {
    #[cfg(feature = "yaml")]
    pub fn from_yaml(value: serde_yaml::Value) -> Result<Value> {
        match value {
            serde_yaml::Value::Null => Ok(Value::Null),
            serde_yaml::Value::Bool(b) => Ok(Value::Boolean(b)),
            serde_yaml::Value::Number(n) => {
                Ok(Value::Integer(n.as_i64().ok_or_else(|| {
                    anyhow!("Cannot convert yaml number to i64")
                })?))
            }
            serde_yaml::Value::String(s) => Ok(Value::String(s)),
            serde_yaml::Value::Sequence(seq) => {
                let seq: Result<Vec<_>> = seq.into_iter().map(Value::from_yaml).collect();
                Ok(Value::Array(seq?))
            }
            serde_yaml::Value::Mapping(dict) => {
                let dict: Result<HashMap<String, _>> = dict
                    .into_iter()
                    .map(|(k, v)| Ok((Value::from_yaml(k)?.try_to_string()?, Value::from_yaml(v)?)))
                    .collect();
                Ok(Value::Dict(dict?))
            }
            serde_yaml::Value::Tagged(v) => Value::from_yaml(v.value),
        }
    }

    #[cfg(feature = "json")]
    pub fn from_json(value: serde_json::Value) -> Result<Value> {
        match value {
            serde_json::Value::Null => Ok(Value::Null),
            serde_json::Value::Bool(b) => Ok(Value::Boolean(b)),
            serde_json::Value::Number(n) => {
                Ok(Value::Integer(n.as_i64().ok_or_else(|| {
                    anyhow!("Cannot convert yaml number to i64")
                })?))
            }
            serde_json::Value::String(s) => Ok(Value::String(s)),
            serde_json::Value::Array(seq) => {
                let seq: Result<Vec<_>> = seq.into_iter().map(Value::from_json).collect();
                Ok(Value::Array(seq?))
            }
            serde_json::Value::Object(dict) => {
                let dict: Result<HashMap<String, _>> = dict
                    .into_iter()
                    .map(|(k, v)| Ok((k, Value::from_json(v)?)))
                    .collect();
                Ok(Value::Dict(dict?))
            }
        }
    }

    #[cfg(feature = "json")]
    pub fn to_json(self) -> serde_json::Value {
        match self {
            Value::String(s) => serde_json::Value::String(s),
            Value::Boolean(b) => serde_json::Value::Bool(b),
            Value::Integer(i) => serde_json::Value::Number(i.into()),
            Value::Array(seq) => {
                serde_json::Value::Array(seq.into_iter().map(Value::to_json).collect())
            }
            Value::Dict(dict) => {
                serde_json::Value::Object(dict.into_iter().map(|(k, v)| (k, v.to_json())).collect())
            }
            Value::Null => serde_json::Value::Null,
        }
    }

    pub fn try_to_string(self) -> Result<String> {
        match self {
            Value::String(s) => Ok(s),
            Value::Boolean(b) => Ok(b.to_string()),
            Value::Integer(i) => Ok(i.to_string()),
            Value::Array(_) => Err(anyhow!("Array cannot be converted to string")),
            Value::Dict(_) => Err(anyhow!("Dict cannot be converted to string")),
            Value::Null => Ok("null".to_string()),
        }
    }

    pub fn typename(&self) -> ValueType {
        match self {
            Value::String(_) => ValueType::String,
            Value::Boolean(_) => ValueType::Boolean,
            Value::Integer(_) => ValueType::Integer,
            Value::Array(_) => ValueType::Array,
            Value::Dict(_) => ValueType::Dict,
            Value::Null => ValueType::Null,
        }
    }
}
