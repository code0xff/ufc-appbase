use std::sync::{Arc, Mutex};

use appbase::*;
use mysql::*;
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;

pub struct MySqlPlugin {
    base: PluginBase,
    pool: Option<MySqlPool>,
    monitor: Option<SubscribeHandle>,
}

appbase_plugin_requires!(MySqlPlugin; JsonRpcPlugin);

type MySqlPool = Arc<Mutex<Pool>>;

impl MySqlPlugin {
    pub fn gen_msg(method: String, value: Value) -> Value {
        let mut message = Map::new();
        message.insert(String::from("method"), Value::String(method));
        message.insert(String::from("value"), value);
        Value::Object(message)
    }
}

impl Plugin for MySqlPlugin {
    appbase_plugin_default!(MySqlPlugin);

    fn new() -> Self {
        MySqlPlugin {
            base: PluginBase::new(),
            monitor: None,
            pool: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        let opts = Opts::from_url("mysql://root:mariadb@localhost:3306/ufc").unwrap();
        let pool = Pool::new(opts).unwrap();
        self.pool = Some(Arc::new(Mutex::new(pool)));
        self.monitor = Some(app::subscribe_channel(String::from("mysql")));
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        // let pool = Arc::clone(self.pool.as_ref().unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                if let Ok(_) = locked_monitor.try_recv() {
                    // let map = message.as_object().unwrap();
                    // let method = map.get("method").unwrap().as_str().unwrap().to_string();
                    // let value = map.get("value").unwrap();
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
