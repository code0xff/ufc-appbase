use serde_json::{Map, Value};

pub fn verify(params: &Map<String, Value>) -> Result<String, String> {
    let key = params.get("key");
    if key.is_none() || !key.unwrap().is_string() {
        return Err(String::from("invalid key value"));
    }
    Ok(String::from("valid"))
}
