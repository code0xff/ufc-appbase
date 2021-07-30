use serde_json::{Map, Value};

use crate::validation::verify::verify_default;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    let verified = verify_default(params, vec![("key", "string")]);
    if verified.is_err() {
        return Err(verified.unwrap_err());
    }
    Ok(())
}

#[cfg(test)]
mod find_by_key_test {
    use serde_json::{json, Map};

    use crate::validation::find_by_key::verify;

    #[test]
    fn verify_test_success() {
        let mut params = Map::new();
        params.insert(String::from("key"), json!("task:tendermint:block:cosmoshub-4"));
        let result = verify(&params);

        assert!(result.is_ok());
    }

    #[test]
    fn verify_test_type_error() {
        let mut params = Map::new();
        params.insert(String::from("key"), json!(1));
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
