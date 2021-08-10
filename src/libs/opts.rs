use appbase::app;

use crate::error::error::ExpectedError;

pub fn bool(key: &str) -> Result<bool, ExpectedError> {
    Ok(app::is_present(key))
}

pub fn string(key: &str) -> Result<String, ExpectedError> {
    let string = app::value_of(key)
        .ok_or(ExpectedError::NoneError("argument is null!".to_string()))?;
    Ok(string.to_string())
}
