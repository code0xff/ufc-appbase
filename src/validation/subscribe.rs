use serde_json::{Map, Value};

pub fn verify(params: &Map<String, Value>) -> Result<String, String> {
    let chain_id = params.get("chain_id");
    if chain_id.is_none() || !chain_id.unwrap().is_string() {
        return Err(String::from("invalid chain_id value"));
    }
    let start_height = params.get("start_height");
    if start_height.is_none() || !start_height.unwrap().is_number() {
        return Err(String::from("invalid start_height value"));
    }
    let nodes = params.get("nodes");
    if nodes.is_none() || !nodes.unwrap().is_array() {
        return Err(String::from("invalid nodes value"));
    }
    Ok(String::from("valid"))
}
