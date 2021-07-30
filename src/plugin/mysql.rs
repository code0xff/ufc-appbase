use std::sync::{Arc, Mutex};

use appbase::*;
use mysql::*;
use mysql::prelude::Queryable;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{enumeration, message};
use crate::libs::mysql_helper::get_params;
use crate::libs::serde_helper::{get_object, get_str};
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::types::enumeration::Enumeration;

pub struct MySqlPlugin {
    base: PluginBase,
    pool: Option<MySqlPool>,
    monitor: Option<SubscribeHandle>,
}

type MySqlPool = Arc<Mutex<Pool>>;

message!((MySqlMsg; {query: String}, {value: Value}); (MySqlMethod; {Insert: "insert"}, {Update: "update"}, {Delete: "delete"}));

appbase_plugin_requires!(MySqlPlugin; JsonRpcPlugin);

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
        let pool = Arc::clone(self.pool.as_ref().unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                if let Ok(message) = locked_monitor.try_recv() {
                    let map = message.as_object().unwrap();
                    let method = MySqlMethod::find(get_str(map, "method").unwrap()).unwrap();
                    match method {
                        MySqlMethod::Insert => {
                            let query = get_str(map, "query").unwrap();
                            let value = get_object(map, "value").unwrap();
                            let params = get_params(value);
                            let result = pool.lock().unwrap().get_conn().unwrap().exec_drop(query, params);
                            if result.is_err() {
                                println!("{}", result.unwrap_err());
                            }
                        }
                        MySqlMethod::Update => {}
                        MySqlMethod::Delete => {}
                    };
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
