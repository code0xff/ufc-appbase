#[macro_export]
macro_rules! get_str {
    ($params:ident; $name:expr) => {
        $params.get($name).unwrap().as_str().unwrap()
    }
}

#[macro_export]
macro_rules! get_string {
    ($params:ident; $name:expr) => {
        String::from(get_str!($params; $name))
    }
}

#[macro_export]
macro_rules! get_u64 {
    ($params:ident; $name:expr) => {
       $params.get($name).unwrap().as_u64().unwrap()
    }
}

#[macro_export]
macro_rules! get_string_vec {
    ($params:ident; $name:expr) => {
        $params.get($name).unwrap().as_array().unwrap().iter().map(|item| { String::from(item.as_str().unwrap()) }).collect()
    }
}

#[macro_export]
macro_rules! get_object {
    ($params:ident; $name:expr) => {
          $params.get($name).unwrap().as_object().unwrap()
    }
}

#[cfg(test)]
mod get_str_test {
    use serde_json::{Map, json};

    #[test]
    fn get_str_macro_test() {
        let mut params = Map::new();
        params.insert(String::from("key"), json!("value"));

        let value = get_str!(params; "key");
        assert_eq!(value, "value");
    }
}
