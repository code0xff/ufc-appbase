use jsonrpc_core::Value;
use serde_json::Map;

pub fn get_params(params: &Map<String, Value>) -> mysql::Params {
    let mut vec: Vec<(String, mysql::Value)> = Vec::new();
    for (key, value) in params.iter() {
        match value {
            Value::Null => vec.push((key.clone(), mysql::Value::NULL)),
            Value::Bool(v) => vec.push((key.clone(), mysql::Value::from(v))),
            Value::Number(v) => {
                if v.is_u64() {
                    vec.push((key.clone(), mysql::Value::UInt(v.as_u64().unwrap())));
                } else if v.is_i64() {
                    vec.push((key.clone(), mysql::Value::Int(v.as_i64().unwrap())));
                } else if v.is_f64() {
                    vec.push((key.clone(), mysql::Value::Double(v.as_f64().unwrap())));
                }
            },
            Value::String(v) => vec.push((key.clone(), mysql::Value::Bytes(v.as_bytes().to_vec()))),
            _ => vec.push((key.clone(), mysql::Value::Bytes(value.to_string().as_bytes().to_vec()))),
        }
    }
    mysql::Params::from(vec)
}
