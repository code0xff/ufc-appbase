use std::sync::{Arc, Mutex};

use appbase::*;
use jsonrpc_core::{Params, serde_from_str};
use rocksdb::{DB, DBWithThreadMode, SingleThreaded};
use serde_json::{json, Map, Value};
use serde::{Serialize, Deserialize};

use crate::{enumeration, get_str, message};
use crate::libs::serialize;
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::types::enumeration::Enumeration;
use crate::validation::{find_by_key, get_task};

pub struct RocksPlugin {
    base: PluginBase,
    db: Option<RocksDB>,
    monitor: Option<SubscribeHandle>,
}

message!((RocksMsg; {key: String}, {value: Value}); (RocksMethod; {Put: "put"}, {Delete: "delete"}));

type RocksDB = Arc<Mutex<DBWithThreadMode<SingleThreaded>>>;

impl RocksPlugin {
    pub fn find_by_prefix(&self, prefix: &str) -> Value {
        let db = Arc::clone(self.db.as_ref().unwrap());
        Self::find_by_prefix_static(&db, prefix)
    }

    fn find_by_prefix_static(db: &RocksDB, prefix: &str) -> Value {
        let db_lock = db.lock().unwrap();
        let mut iter = db_lock.raw_iterator();
        iter.seek(prefix.as_bytes());
        let mut result: Vec<Value> = Vec::new();
        while iter.valid() && serialize::deserialize(iter.key().unwrap()).starts_with(prefix) {
            let value: Map<String, Value> = serde_from_str(serialize::deserialize(iter.value().unwrap()).as_str()).unwrap();
            result.push(Value::Object(value));
            iter.next();
        }
        Value::Array(result)
    }

    pub fn find_by_key(db: &RocksDB, key: &str) -> Value {
        let db_lock = db.lock().unwrap();
        let result = db_lock.get(key.as_bytes()).unwrap();
        match result {
            None => {
                Value::Null
            }
            Some(value) => {
                Value::Object(serde_json::from_str(String::from_utf8(value).unwrap().as_str()).unwrap())
            }
        }
    }
}

appbase_plugin_requires!(RocksPlugin; );

impl Plugin for RocksPlugin {
    appbase_plugin_default!(RocksPlugin);

    fn new() -> Self {
        RocksPlugin {
            base: PluginBase::new(),
            db: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        self.db = Some(Arc::new(Mutex::new(DB::open_default("rocks").unwrap())));
        self.monitor = Some(app::subscribe_channel(String::from("rocks")));

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let db = Arc::clone(self.db.as_ref().unwrap());
        jsonrpc.add_method(String::from("get_tasks"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_task::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let prefix = match params.get("task_id") {
                None => {
                    "task"
                }
                Some(task_id) => {
                    task_id.as_str().unwrap()
                }
            };
            let tasks = Self::find_by_prefix_static(&db, prefix);
            Box::new(futures::future::ready(Ok(tasks)))
        });

        let db = Arc::clone(self.db.as_ref().unwrap());
        jsonrpc.add_method(String::from("find_by_key"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = find_by_key::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let key = get_str!(params; "key");
            let value = Self::find_by_key(&db, key);
            Box::new(futures::future::ready(Ok(value)))
        });
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let db = Arc::clone(self.db.as_ref().unwrap());
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                let db_lock = db.lock().unwrap();
                if let Ok(msg) = mon_lock.try_recv() {
                    let data = msg.as_object().unwrap();
                    let method = RocksMethod::find(data.get("method").unwrap().as_str().unwrap()).unwrap();
                    match method {
                        RocksMethod::Put => {
                            let key = get_str!(data; "key");
                            let val = get_str!(data; "value");
                            let _ = db_lock.put(key.as_bytes(), val.as_bytes());
                        }
                        RocksMethod::Delete => {
                            let key = get_str!(data; "key");
                            let _ = db_lock.delete(key.as_bytes());
                        }
                    }
                }
            }
        });
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
    }
}
