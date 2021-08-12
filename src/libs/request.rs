use serde_json::{Map, Value};

use crate::error::error::ExpectedError;
use crate::libs::serde::get_string;

pub fn get(url: &str) -> Result<Map<String, Value>, ExpectedError> {
    let res = reqwest::blocking::get(url)?;
    let status = res.status().clone();
    let body = res.text()?;
    let parsed_body: Map<String, Value> = serde_json::from_str(body.as_str())?;

    if !status.is_success() {
        let error = get_string(&parsed_body, "error")?;
        return Err(ExpectedError::RequestError(error));
    }
    Ok(parsed_body)
}
