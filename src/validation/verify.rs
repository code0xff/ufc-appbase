use serde_json::{Map, Value};

use crate::libs::serde::get_type;

pub fn verify_default(params: &Map<String, Value>, names: Vec<(&str, &str)>) -> Result<(), String> {
    for (name, types) in names.into_iter() {
        if params.get(name).is_none() {
            return Err(format!("{} does not exist", name));
        } else {
            let checked_type = get_type(params.get(name).unwrap());
            if checked_type != types {
                return Err(format!("{} is not {}", name, types));
            }
        }
    }
    Ok(())
}
