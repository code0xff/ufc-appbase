use jsonrpc_core::Value;
use serde_json::Map;

use crate::error::error::ExpectedError;

pub fn get_params(params: &Map<String, Value>) -> mysql::Params {
    let mut vec: Vec<mysql::Value> = Vec::new();
    for (_, value) in params.iter() {
        match value {
            Value::Null => vec.push(mysql::Value::NULL),
            Value::Bool(v) => vec.push(mysql::Value::from(v)),
            Value::Number(v) => {
                if v.is_u64() {
                    vec.push(mysql::Value::UInt(v.as_u64().unwrap()));
                } else if v.is_i64() {
                    vec.push(mysql::Value::Int(v.as_i64().unwrap()));
                } else if v.is_f64() {
                    vec.push(mysql::Value::Double(v.as_f64().unwrap()));
                }
            }
            Value::String(v) => vec.push(mysql::Value::Bytes(v.as_bytes().to_vec())),
            _ => vec.push(mysql::Value::Bytes(value.to_string().as_bytes().to_vec())),
        }
    }
    mysql::Params::from(vec)
}

pub fn convert_type(_type: String, max_length: Option<u32>) -> Result<String, ExpectedError> {
    let converted = if _type == "string" {
        if max_length.is_some() && max_length.unwrap() <= 65535 {
            "varchar"
        } else {
            "text"
        }
    } else if _type == "integer" {
        if max_length.is_some() && max_length.unwrap() <= 11 {
            "int"
        } else {
            "bigint"
        }
    } else if _type == "number" {
        "double"
    } else if _type == "boolean" {
        "boolean"
    } else if _type == "object" {
        "json"
    } else if _type == "array" {
        "text"
    } else {
        return Err(ExpectedError::TypeError(String::from("unsupported type!")));
    };
    Ok(String::from(converted))
}