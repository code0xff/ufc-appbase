use serde_json::{Map, Value};

use crate::error::error::ExpectedError;
use crate::validation::verify::verify_default;

pub fn verify(params: &Map<String, Value>) -> Result<(), ExpectedError> {
    verify_default(params, vec![("task_id", "string")])?;
    Ok(())
}
