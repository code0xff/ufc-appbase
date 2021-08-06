use std::sync::Arc;

use appbase::*;
use jsonrpc_core::Params;
use rocksdb::{DB, DBWithThreadMode, SingleThreaded};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{enumeration, message};
use crate::libs::serde::get_str;
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::types::enumeration::Enumeration;
use crate::validation::find_by_key;

pub struct RocksPlugin {
    db: Option<RocksDB>,
    monitor: Option<SubscribeHandle>,
}

type RocksDB = Arc<DBWithThreadMode<SingleThreaded>>;

message!((RocksMsg; {key: String}, {value: Value}); (RocksMethod; {Put: "put"}, {Delete: "delete"}));

appbase_plugin_requires!(RocksPlugin; );

impl Plugin for RocksPlugin {
    fn new() -> Self {
        RocksPlugin {
            db: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        self.db = Some(Arc::new(DB::open_default("rocks").unwrap()));
        self.monitor = Some(app::subscribe_channel(String::from("rocks")));

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let db = self.db.as_ref().unwrap().clone();
        jsonrpc.add_method(String::from("find_by_key"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = find_by_key::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let key = get_str(&params, "key").unwrap();
            let value = Self::find_by_prefix_static(&db, key);
            Box::new(futures::future::ready(Ok(value)))
        });
    }

    fn startup(&mut self) {
        let db = self.db.as_ref().unwrap().clone();
        let monitor = self.monitor.as_ref().unwrap().clone();
        let app = app::quit_handle().unwrap();
        Self::recv(db, monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl RocksPlugin {
    pub fn find_by_prefix_static(db: &RocksDB, key: &str) -> Value {
        let result = db.get(key.as_bytes()).unwrap();
        match result {
            None => Value::Null,
            Some(value) => {
                Value::Object(serde_json::from_str(String::from_utf8(value).unwrap().as_str()).unwrap())
            }
        }
    }

    pub fn get_db(&self) -> Arc<DBWithThreadMode<SingleThreaded>> {
        self.db.as_ref().unwrap().clone()
    }

    fn recv(db: RocksDB, monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let method = RocksMethod::find(parsed_msg.get("method").unwrap().as_str().unwrap()).unwrap();
                    match method {
                        RocksMethod::Put => {
                            let key = get_str(parsed_msg, "key").unwrap();
                            let val = get_str(parsed_msg, "value").unwrap();
                            let _ = db.put(key.as_bytes(), val.as_bytes());
                        }
                        RocksMethod::Delete => {
                            let key = get_str(parsed_msg, "key").unwrap();
                            let _ = db.delete(key.as_bytes());
                        }
                    }
                }
            }
            if !app.is_quiting() {
                Self::recv(db, monitor, app);
            }
        });
    }
}
