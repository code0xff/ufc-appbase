use serde_json::{Map, Value};

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    if params.get("task_id").is_some() && !params.get("task_id").unwrap().is_string() {
        return Err(String::from("invalid task_id value"));
    }
    Ok(())
}
