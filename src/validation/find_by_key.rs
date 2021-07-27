use serde_json::{Map, Value};
use crate::verify_str;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    verify_str!(params; "key");
    Ok(())
}
