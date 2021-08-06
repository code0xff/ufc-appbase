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
        if value.is_u64() {
            vec.push((key.clone(), mysql::Value::UInt(value.as_u64().unwrap())));
        } else if value.is_i64() {
            vec.push((key.clone(), mysql::Value::Int(value.as_i64().unwrap())));
        } else if value.is_f64() {
            vec.push((key.clone(), mysql::Value::Double(value.as_f64().unwrap())));
        } else if value.is_null() {
            vec.push((key.clone(), mysql::Value::NULL));
        } else if value.is_null() {
            vec.push((key.clone(), mysql::Value::NULL));
        } else if value.is_string() {
            vec.push((key.clone(), mysql::Value::Bytes(value.as_str().unwrap().as_bytes().to_vec())));
        } else {
            vec.push((key.clone(), mysql::Value::Bytes(value.to_string().as_bytes().to_vec())));
        }
    }
    mysql::Params::from(vec)
}
