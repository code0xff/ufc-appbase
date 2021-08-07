use std::env;
use std::str::FromStr;

use crate::error::error::ExpectedError;

pub fn bool(key: &str) -> Result<bool, ExpectedError> {
    let string = string(key)?;
    let bool = bool::from_str(string.as_str())?;
    Ok(bool)
}

pub fn string(key: &str) -> Result<String, ExpectedError> {
    let string = env::var(key)?;
    Ok(string)
}
