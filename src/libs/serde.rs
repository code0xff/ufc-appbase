use serde_json::{Map, Value};

use crate::error::error::ExpectedError;

pub fn select_value(params: &Map<String, Value>, names: Vec<&str>) -> Result<Map<String, Value>, ExpectedError> {
    let mut values = Map::new();
    for name in names.into_iter() {
        let value = find_value(params, name);
        values.insert(String::from(name), value);
    }
    Ok(values)
}

pub fn find_value(values: &Map<String, Value>, target_name: &str) -> Value {
    if values.get(target_name).is_some() {
        values.get(target_name).unwrap().clone()
    } else {
        for (_, value) in values.iter() {
            match value {
                Value::Object(value_obj) => {
                    let ret_value = find_value(value_obj, target_name);
                    if ret_value.is_null() {
                        continue;
                    } else {
                        return ret_value;
                    }
                }
                Value::Array(value_vec) => {
                    for element in value_vec {
                        if element.is_object() {
                            let ret_value = find_value(element.as_object().unwrap(), target_name);
                            if ret_value.is_null() {
                                continue;
                            } else {
                                return ret_value;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Value::Null
    }
}

pub fn unwrap<'a>(params: &'a Map<String, Value>, name: &'a str) -> Result<&'a Value, ExpectedError> {
    let opt_val = params.get(name);
    match opt_val {
        None => Err(ExpectedError::NoneError(format!("{} does not exist!", name))),
        Some(val) => Ok(val),
    }
}

pub fn get_str<'a>(params: &'a Map<String, Value>, name: &'a str) -> Result<&'a str, ExpectedError> {
    let unwrapped = unwrap(params, name)?;
    let opt_val = unwrapped.as_str();
    match opt_val {
        None => Err(ExpectedError::NoneError(format!("{} is not {}!", name, "str"))),
        Some(val) => Ok(val),
    }
}

pub fn get_string(params: &Map<String, Value>, name: &str) -> Result<String, ExpectedError> {
    let result = get_str(params, name)?;
    Ok(String::from(result))
}

pub fn get_u64(params: &Map<String, Value>, name: &str) -> Result<u64, ExpectedError> {
    let unwrapped = unwrap(params, name)?;
    let opt_val = unwrapped.as_u64();
    match opt_val {
        None => Err(ExpectedError::TypeError(format!("{} is not {}!", name, "u64"))),
        Some(val) => Ok(val),
    }
}

pub fn get_object<'a>(params: &'a Map<String, Value>, name: &'a str) -> Result<&'a Map<String, Value>, ExpectedError> {
    let unwrapped = unwrap(params, name)?;
    let opt_val = unwrapped.as_object();
    match opt_val {
        None => Err(ExpectedError::TypeError(format!("{} is not {}!", name, "object"))),
        Some(val) => Ok(val),
    }
}

pub fn get_array<'a>(params: &'a Map<String, Value>, name: &'a str) -> Result<&'a Vec<Value>, ExpectedError> {
    let unwrapped = unwrap(params, name)?;
    let opt_val = unwrapped.as_array();
    match opt_val {
        None => Err(ExpectedError::TypeError(format!("{} is not {}!", name, "array"))),
        Some(val) => Ok(val),
    }
}

// pub fn get_bool(params: &Map<String, Value>, name: &str) -> Result<bool, ExpectedError> {
//     let unwrapped = unwrap(params, name)?;
//     let opt_val = unwrapped.as_bool();
//     match opt_val {
//         None => Err(ExpectedError::TypeError(format!("{} is not {}!", name, "bool"))),
//         Some(val) => Ok(val),
//     }
// }

pub fn get_value_by_path<'a>(params: &'a Map<String, Value>, path: &'a str) -> Result<&'a Value, ExpectedError> {
    let split = path.split(".");
    if split.clone().count() == 0 {
        return Err(ExpectedError::InvalidError(String::from("path cannot be empty!")));
    }
    let mut params = params;
    let last = split.clone().last().unwrap();
    for name in split {
        if name == last {
            let target = unwrap(params, name)?;
            return Ok(target);
        } else {
            params = get_object(params, name)?;
        }
    }
    Err(ExpectedError::NoneError(format!("value does not exist in the path! path={}", path)))
}

pub fn get_string_vec(params: &Map<String, Value>, name: &str) -> Vec<String> {
    params.get(name).unwrap().as_array().unwrap().iter().map(|item| { String::from(item.as_str().unwrap()) }).collect()
}

pub fn get_type(value: &Value) -> String {
    let types = match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(v) => {
            if v.is_u64() {
                "u64"
            } else if v.is_i64() {
                "i64"
            } else if v.is_f64() {
                "f64"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    String::from(types)
}

pub fn filter(values: &Map<String, Value>, filter: String) -> Result<bool, ExpectedError> {
    if filter.trim().is_empty() {
        return Ok(true);
    }
    let mut calc_vec: Vec<String> = Vec::new();
    let mut key_value = String::new();
    let filter_chars = filter.chars();
    for c in filter_chars {
        if c == '&' || c == '|' {
            calc_vec.push(key_value.clone());
            calc_vec.push(String::from(c));
            key_value = String::new();
        } else {
            key_value.push(c);
        }
    }
    calc_vec.push(key_value.clone());

    let mut ret = filter_calc(values, calc_vec.first().unwrap())?;
    let mut calc_iter = calc_vec.iter().skip(1);
    while let Some(and_or) = calc_iter.next() {
        match calc_iter.next() {
            None => {
                return Err(ExpectedError::NoneError(String::from("the size of filter condition is not insufficient!")));
            }
            Some(key_value) => {
                let value = filter_calc(values, key_value)?;
                if and_or == "&" {
                    ret &= value;
                } else {
                    ret |= value;
                }
            }
        };
    }
    Ok(ret)
}

fn filter_calc(values: &Map<String, Value>, key_value: &String) -> Result<bool, ExpectedError> {
    let mut split_kv = key_value.split("=");
    if split_kv.clone().count() < 2 {
        return Err(ExpectedError::TypeError(String::from("filter condition must contain '='!")));
    }
    let key = split_kv.next().unwrap().trim();
    let value = split_kv.next().unwrap().trim();
    let found = if key.contains(".") {
        match get_value_by_path(values, key) {
            Ok(val) => val.clone(),
            Err(_) => Value::Null
        }
    } else {
        find_value(values, key)
    };
    let found_val = match found {
        Value::String(s) => s,
        _ => found.to_string(),
    };
    Ok(value == found_val.as_str())
}

#[cfg(test)]
mod serde {
    use serde_json::{json, Map, Value};

    use crate::libs::serde;

    #[test]
    fn filter_success_test() {
        let mut test_map = Map::new();
        test_map.insert(String::from("key1"), Value::String(String::from("val1")));
        test_map.insert(String::from("key2"), json!({"sub_key1": "sub_val1"}));
        test_map.insert(String::from("key3"), json!(100));

        let ret = serde::filter(&test_map, String::from("key1 = val1 & sub_key1 = sub_val1 & key3 =101 | key4=null")).unwrap();
        assert_eq!(ret, true);
    }

    #[test]
    fn filter_fail_test() {
        let mut test_map = Map::new();
        test_map.insert(String::from("key1"), Value::String(String::from("val1")));
        test_map.insert(String::from("key2"), json!({"sub_key1": "sub_val1"}));
        test_map.insert(String::from("key3"), json!(100));
        test_map.insert(String::from("key4"), Value::String(String::from("not_null")));

        let ret = serde::filter(&test_map, String::from("key1 = val1 & sub_key1 = sub_val1 & key3 =100 & key4=null")).unwrap();
        assert_eq!(ret, false);
    }
}
