#[macro_export]
macro_rules! unwrap {
    ($params:expr; $name:expr) => {
        {
            let opt_val = $params.get($name);
            match opt_val {
                None => {
                    Err(format!("{} does not exist", $name))
                }
                Some(val) => {
                    Ok(val)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! get_str {
    ($params:expr; $name:expr) => {
        {
            let unwrapped = unwrap!($params; $name);
            if unwrapped.is_ok() {
                let opt_val = unwrapped.unwrap().as_str();
                match opt_val {
                    None => {
                        Err(format!("{} is not {}", $name, "str"))
                    }
                    Some(val) => {
                        Ok(val)
                    }
                }
            } else {
                Err(unwrapped.unwrap_err())
            }
        };
    }
}

#[macro_export]
macro_rules! get_string {
    ($params:expr; $name:expr) => {
        {
            let unwrapped = unwrap!($params; $name);
            if unwrapped.is_ok() {
                let opt_val = unwrapped.unwrap().as_str();
                match opt_val {
                    None => {
                        Err(format!("{} is not {}", $name, "string"))
                    }
                    Some(val) => {
                        Ok(String::from(val))
                    }
                }
            } else {
                Err(unwrapped.unwrap_err())
            }
        }
    };
}

#[macro_export]
macro_rules! get_u64 {
    ($params:expr; $name:expr) => {
        {
            let unwrapped = unwrap!($params; $name);
            if unwrapped.is_ok() {
                let opt_val = unwrapped.unwrap().as_u64();
                match opt_val {
                    None => {
                        Err(format!("{} is not {}", $name, "u64"))
                    }
                    Some(val) => {
                        Ok(val)
                    }
                }
            } else {
                Err(unwrapped.unwrap_err())
            }
        }
    }
}

#[macro_export]
macro_rules! get_string_vec {
    ($params:expr; $name:expr) => {
        $params.get($name).unwrap().as_array().unwrap().iter().map(|item| { String::from(item.as_str().unwrap()) }).collect()
    }
}

#[macro_export]
macro_rules! get_object {
    ($params:expr; $name:expr) => {
        {
            let unwrapped = unwrap!($params; $name);
            if unwrapped.is_ok() {
                let opt_val = unwrapped.unwrap().as_object();
                match opt_val {
                    None => {
                        Err(format!("{} is not {}", $name, "object"))
                    }
                    Some(val) => {
                        Ok(val)
                    }
                }
            } else {
                Err(unwrapped.unwrap_err())
            }
        }
    }
}

#[cfg(test)]
mod get_str_test {
    use serde_json::{json, Map};

    #[test]
    fn get_str_macro_test() {
        let mut params = Map::new();
        params.insert(String::from("key"), json!("value"));

        let value = get_str!(params; "key");
        assert_eq!(value, "value");
    }
}
