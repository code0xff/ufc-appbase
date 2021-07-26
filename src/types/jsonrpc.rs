use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    jsonrpc: String,
    id: i32,
    method: String,
    params: Map<String, Value>,
}

impl JsonRpcRequest {
    pub fn new(id: i32, method: String, params: Map<String, Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: String::from("2.0"),
            id,
            method,
            params
        }
    }

    pub fn params(height: u64) -> Map<String, Value> {
        let mut params = Map::new();
        params.insert(String::from("query"), Value::String(format!("tx.height={}", height)));
        params.insert(String::from("page"), Value::String(String::from("1")));
        params.insert(String::from("per_page"), Value::String(String::from("100")));
        params.insert(String::from("order_by"), Value::String(String::from("asc")));
        params
    }
}
