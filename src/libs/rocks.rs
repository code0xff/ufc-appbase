use std::sync::Arc;

use jsonrpc_core::serde_from_str;
use rocksdb::{DBWithThreadMode, SingleThreaded};
use serde_json::{Map, Value};

use crate::libs::serialize;

type RocksDB = Arc<DBWithThreadMode<SingleThreaded>>;

pub fn get_static(rocksdb: &RocksDB, key: &str) -> Value {
    let result = rocksdb.get(key.as_bytes()).unwrap();
    match result {
        None => {
            Value::Null
        }
        Some(value) => {
            Value::Object(serde_json::from_str(String::from_utf8(value).unwrap().as_str()).unwrap())
        }
    }
}

pub fn put_static(rocksdb: &RocksDB, key: &str, value: &str) {
    let _ = rocksdb.put(key.as_bytes(), value.as_bytes());
}

pub fn delete_static(rocksdb: &RocksDB, key: &str) {
    let _ = rocksdb.delete(key.as_bytes());
}

pub fn get_by_prefix_static(rocksdb: &RocksDB, prefix: &str) -> Value {
    let mut iter = rocksdb.raw_iterator();
    iter.seek(prefix.as_bytes());
    let mut result: Vec<Value> = Vec::new();
    while iter.valid() && serialize::deserialize(iter.key().unwrap()).starts_with(prefix) {
        let value: Map<String, Value> = serde_from_str(serialize::deserialize(iter.value().unwrap()).as_str()).unwrap();
        result.push(Value::Object(value));
        iter.next();
    }
    Value::Array(result)
}
