use serde_json::{Map, Value};
use crate::types::subscribe::SubscribeTarget;

pub fn verify(params: &Map<String, Value>) -> Result<String, String> {
    let target = params.get("target");
    if target.is_none() || !target.unwrap().is_string() || !SubscribeTarget::valid(target.unwrap().as_str().unwrap()) {
        return Err(String::from("invalid target value"));
    }
    let sub_id = params.get("sub_id");
    if sub_id.is_none() || !sub_id.unwrap().is_string() {
        return Err(String::from("invalid sub_id value"));
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
