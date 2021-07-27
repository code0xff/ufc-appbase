use serde_json::{Map, Value};

use crate::{verify_arr, verify_num, verify_str};
use crate::types::subscribe::SubscribeTarget;
use crate::types::enumeration::Enumeration;

pub fn verify(params: &Map<String, Value>) -> Result<(), String> {
    verify_str!(params; "target", "sub_id");
    verify_num!(params; "start_height");
    verify_arr!(params; "nodes");
    if !SubscribeTarget::valid(params.get("target").unwrap().as_str().unwrap()) {
        return Err(String::from("matched target does not exit"));
    }
    Ok(())
}

#[cfg(test)]
mod subscribe_test {
    use serde_json::{Map, json};
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
