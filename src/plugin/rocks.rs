use std::sync::{Arc, Mutex};

use appbase::*;
use jsonrpc_core::{Params, serde_from_str};
use rocksdb::{DB, DBWithThreadMode, SingleThreaded};
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::validation::get_task;

pub struct RocksPlugin {
    base: PluginBase,
    db: Option<RocksDB>,
    monitor: Option<SubscribeHandle>,
}

type RocksDB = Arc<Mutex<DBWithThreadMode<SingleThreaded>>>;

impl RocksPlugin {
    pub fn gen_msg(method: String, key: String, value: Option<Value>) -> Value {
        let mut msg = Map::new();
        msg.insert(String::from("method"), Value::String(method));
        msg.insert(String::from("key"), Value::String(key));
        match value {
            None => {}
            Some(v) => {
                msg.insert(String::from("value"), v);
            }
        }
        Value::Object(msg)
    }

    pub fn find_by_prefix(&self, prefix: &str) -> Value {
        let db = Arc::clone(self.db.as_ref().unwrap());
        Self::find_by_prefix_static(&db, prefix)
    }

    fn find_by_prefix_static(db: &RocksDB, prefix: &str) -> Value {
        let db_lock = db.lock().unwrap();
        let mut iter = db_lock.raw_iterator();
        iter.seek(prefix.as_bytes());
        let mut result: Vec<Value> = vec![];
        while iter.valid() && String::from_utf8(iter.key().unwrap().to_vec()).unwrap().starts_with("task") {
            let value: Map<String, Value> = serde_from_str(String::from_utf8(iter.value().unwrap().to_vec()).unwrap().as_str()).unwrap();
            result.push(Value::Object(value));
            iter.next();
        }
        Value::Array(result)
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
            let tasks = Self::find_by_prefix_static(&db, "task");
            Box::new(futures::future::ready(Ok(tasks)))
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
                    let method = String::from(data.get("method").unwrap().as_str().unwrap());
                    if method == String::from("put") {
                        let key = data.get("key").unwrap().as_str().unwrap();
                        let val = data.get("value").unwrap().as_str().unwrap();
                        let _ = db_lock.put(key.as_bytes(), val.as_bytes());
                    } else if method == String::from("delete") {
                        let key = data.get("key").unwrap().as_str().unwrap();
                        let _ = db_lock.delete(key.as_bytes());
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
