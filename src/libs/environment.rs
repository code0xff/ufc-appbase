use std::str::FromStr;
use std::env;

pub fn bool(key: &str) -> Result<bool, String> {
    let string_result = string(key);
    if string_result.is_err() {
        return Err(string_result.unwrap_err().to_string());
    }
    let string = string_result.unwrap();
    let bool_result = bool::from_str(string.as_str());
    if bool_result.is_err() {
        return Err(bool_result.unwrap_err().to_string());
    }
    Ok(bool_result.unwrap())
}

pub fn string(key: &str) -> Result<String, String> {
    let string_result = env::var(key);
    if string_result.is_err() {
        return Err(string_result.unwrap_err().to_string());
    }
    Ok(string_result.unwrap())
}
