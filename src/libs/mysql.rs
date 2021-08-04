use jsonrpc_core::Value;
use serde_json::Map;

pub fn insert_query(table: &str, mut column: Vec<&str>) -> String {
    let columns = column.join(", ");
    let converted: Vec<String> = column.iter_mut().map(|v| { format!(":{}", v) }).collect();
    let values = converted.join(", ");
    format!("insert into {} ({}) values ({})", table, columns, values)
}

pub fn get_params(params: &Map<String, Value>) -> mysql::Params {
    let mut vec: Vec<(String, mysql::Value)> = Vec::new();
    for (key, value) in params.iter() {
        if value.as_str().is_some() {
            vec.push((key.clone(), mysql::Value::Bytes(value.as_str().unwrap().as_bytes().to_vec())));
        } else if value.as_u64().is_some() {
            vec.push((key.clone(), mysql::Value::UInt(value.as_u64().unwrap())));
        } else {
            vec.push((key.clone(), mysql::Value::NULL));
        }
    }
    mysql::Params::from(vec)
}
