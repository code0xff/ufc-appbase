use serde_json::{Map, Value};

use crate::types::enumeration::Enumeration;
use crate::types::subscribe::SubscribeTarget;
use crate::validation::verify::verify_default;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    let verified = verify_default(params, vec![
        ("target", "string"),
        ("sub_id", "string"),
        ("start_height", "u64"),
        ("nodes", "array"),
    ]);
    if verified.is_err() {
        return Err(verified.unwrap_err());
    }
    if !SubscribeTarget::valid(params.get("target").unwrap().as_str().unwrap()) {
        return Err(String::from("matched target does not exit"));
    }
    Ok(())
}

#[cfg(test)]
mod subscribe_test {
    use serde_json::{json, Map};

    use crate::validation::subscribe::verify;

    #[test]
    fn verify_test_success() {
        let mut params = Map::new();
        params.insert(String::from("target"), json!("block"));
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1));
        params.insert(String::from("nodes"), json!(vec!("https://api.cosmos.network")));
        let result = verify(&params);

        assert!(result.is_ok());
    }

    #[test]
    fn verify_test_type_error() {
        let mut params = Map::new();
        params.insert(String::from("target"), json!(1));
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1));
        params.insert(String::from("nodes"), json!(vec!("https://api.cosmos.network")));
        let result = verify(&params);

        assert!(result.is_err());
    }

    #[test]
    fn verify_test_value_none() {
        let mut params = Map::new();
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1));
        params.insert(String::from("nodes"), json!(vec!("https://api.cosmos.network")));
        let result = verify(&params);

        assert!(result.is_err());
    }
}
