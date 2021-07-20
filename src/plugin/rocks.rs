use std::sync::{Arc, Mutex};

use appbase::*;
use rocksdb::{DB, DBWithThreadMode, SingleThreaded};
use serde_json::{Value, Map};

pub struct RocksPlugin {
    base: PluginBase,
    db: Option<RocksDB>,
    monitor: Option<SubscribeHandle>,
}

impl RocksPlugin {
    pub fn gen_msg(method: String, key: String, value: Value) -> Value {
        let mut data = Map::new();
        data.insert(String::from("method"), Value::String(method));
        data.insert(String::from("key"), Value::String(key));
        data.insert(String::from("value"), value);
        Value::Object(data)
    }
}

type RocksDB = Arc<Mutex<DBWithThreadMode<SingleThreaded>>>;

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
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let db = Arc::clone(self.db.as_ref().unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                let locked_db = db.lock().unwrap();
                if let Ok(message) = locked_monitor.try_recv() {
                    let data = message.as_object().unwrap();
                    let method = String::from(data.get("method").unwrap().as_str().unwrap());
                    if method == String::from("put") {
                        let key = String::from(data.get("key").unwrap().as_str().unwrap());
                        let value = String::from(data.get("value").unwrap().as_str().unwrap());
                        let _ = locked_db.put(key.clone(), value);
                    } else if method == String::from("delete") {
                        let key = String::from(data.get("key").unwrap().as_str().unwrap());
                        let _ = locked_db.delete(key.clone());
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
