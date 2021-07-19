use serde_json::{Map, Value};

pub fn verify(params: &Map<String, Value>) -> Result<String, String> {
    let task_id = params.get("task_id");
    if task_id.is_some() && !task_id.unwrap().is_string() {
        return Err(String::from("invalid task_id value"));
    }
    Ok(String::from("valid"))
}
