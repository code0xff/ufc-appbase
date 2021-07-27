use serde_json::{Map, Value};
use crate::verify_str;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    verify_str!(params; "key");
    Ok(())
}

#[cfg(test)]
mod find_by_key_test {
    use serde_json::{Map, json};
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
