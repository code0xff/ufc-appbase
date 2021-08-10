use serde_json::{Map, Value};

use crate::error::error::ExpectedError;

pub fn pick(params: &Map<String, Value>, names: Vec<&str>) -> Result<Map<String, Value>, ExpectedError> {
    let mut values = Map::new();
    for name in names.into_iter() {
        if params.get(name).is_none() {
            return Err(ExpectedError::NoneError(format!("{} does not belong to map!", name)));
        } else {
            values.insert(String::from(name), params.get(name).unwrap().clone());
        }
    }
    Ok(values)
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

pub fn get_bool(params: &Map<String, Value>, name: &str) -> Result<bool, ExpectedError> {
    let unwrapped = unwrap(params, name)?;
    let opt_val = unwrapped.as_bool();
    match opt_val {
        None => Err(ExpectedError::TypeError(format!("{} is not {}!", name, "bool"))),
        Some(val) => Ok(val),
    }
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
        },
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    String::from(types)
}
