use serde_json::{Map, Value};

use crate::validation::verify::verify_default;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    let verified = verify_default(params, vec![("task_id", "string")]);
    if verified.is_err() {
        return Err(verified.unwrap_err());
    }
    Ok(())
}

#[cfg(test)]
mod unsubscribe_test {
    use serde_json::{json, Map};

    use crate::validation::unsubscribe::verify;

    #[test]
    fn verify_test_success() {
        let mut params = Map::new();
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

    #[test]
    fn verify_test_value_none() {
        let params = Map::new();
        let result = verify(&params);

        assert!(result.is_err());
    }
}
