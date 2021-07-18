use serde_json::{Map, Value};

pub fn verify(params: &Map<String, Value>) -> Result<String, String> {
    let chain = params.get("chain");
    if chain.is_none() || !chain.unwrap().is_string() {
        return Err(String::from("invalid chain value"));
    }
    let target = params.get("target");
    if target.is_none() || !target.unwrap().is_string() {
        return Err(String::from("invalid target value"));
    }
    let sub_id = params.get("sub_id");
    if sub_id.is_some() && sub_id.unwrap().is_u64() {
        return Err(String::from("invalid sub_id value"));
    }
    Ok(String::from("valid"))
}