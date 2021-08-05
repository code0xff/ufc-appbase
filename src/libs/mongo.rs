use mongodb::bson::{Bson, bson, Document};
use serde_json::{Map, Value};

pub fn get_doc(params: &Map<String, Value>) -> Document {
    let mut doc = Document::new();
    for (key, value) in params {
        let bson_value = get_bson(value);
        doc.insert(key, bson_value);
    }
    doc
}

fn get_bson(value: &Value) -> Bson {
    match value {
        Value::Null => Bson::Null,
        Value::Bool(b) => Bson::Boolean(*b),
        Value::Number(n) => {
            if n.is_i64() {
                Bson::Int64(n.as_i64().unwrap())
            } else if n.is_f64() {
                Bson::Double(n.as_f64().unwrap())
            } else {
                bson!(n.to_string())
            }
        }
        Value::String(s) => Bson::String(s.clone()),
        Value::Array(vec) => Bson::Array(vec.iter().map(|v| { get_bson(v) }).collect()),
        Value::Object(obj) => Bson::Document(get_doc(obj)),
    }
}
