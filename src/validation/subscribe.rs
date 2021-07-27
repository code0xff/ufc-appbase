use serde_json::{Map, Value};

use crate::{verify_arr, verify_num, verify_str};
use crate::types::subscribe::SubscribeTarget;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    verify_str!(params; "target", "sub_id");
    verify_num!(params; "start_height");
    verify_arr!(params; "nodes");
    if !SubscribeTarget::valid(params.get("target").unwrap().as_str().unwrap()) {
        return Err(String::from("matched target does not exit"));
    }
    Ok(())
}
