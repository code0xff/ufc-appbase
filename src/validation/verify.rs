use serde_json::{Map, Value};

use crate::libs::serde::get_type;
use crate::error::error::ExpectedError;

pub fn verify_default(params: &Map<String, Value>, names: Vec<(&str, &str)>) -> Result<(), ExpectedError> {
    for (name, types) in names.into_iter() {
        if params.get(name).is_none() {
            return Err(ExpectedError::NoneError(format!("{} does not exist!", name)));
        } else {
            let checked_type = get_type(params.get(name).unwrap());
            if checked_type != types {
                return Err(ExpectedError::TypeError(format!("{} is not {}!", name, types)));
            }
        }
    }
    Ok(())
}
