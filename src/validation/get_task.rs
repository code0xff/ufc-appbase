use serde_json::{Map, Value};

use crate::error::error::ExpectedError;

pub fn verify(params: &Map<String, Value>) -> Result<(), ExpectedError> {
    if params.get("task_id").is_some() && !params.get("task_id").unwrap().is_string() {
        return Err(ExpectedError::TypeError(String::from("invalid task_id value!")));
    }
    Ok(())
}

#[cfg(test)]
mod get_task_test {
    use serde_json::{json, Map};

    use crate::validation::get_task::verify;

    #[test]
    fn verify_test_success() {
        let mut params = Map::new();
        let result = verify(&params);

        assert!(result.is_ok());

        params.insert(String::from("task_id"), json!("task:tendermint:block:cosmoshub-4"));
        let result = verify(&params);

        assert!(result.is_ok());
    }

    #[test]
    fn verify_test_type_error() {
        let mut params = Map::new();
        params.insert(String::from("task_id"), json!(1));
        let result = verify(&params);

        assert!(result.is_err());
    }
}
