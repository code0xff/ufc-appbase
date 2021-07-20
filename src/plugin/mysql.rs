use std::sync::{Arc, Mutex};

use appbase::*;
use jsonrpc_core::Params;
use mysql::*;
use mysql::prelude::*;
use serde_json::{json, Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::types::block::BlockTask;

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

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let pool = Arc::clone(self.pool.as_ref().unwrap());
        jsonrpc.add_method(String::from("get_tasks"), move |params: Params| {
            let mut conn = pool.lock().unwrap().get_conn().unwrap();
            let tasks = conn.query_map(
                r"SELECT task_id, chain, chain_id, start_height, nodes from ufc.block_task",
                |(task_id, chain, chain_id, start_height, nodes)| {
                    let nodes = serde_json::from_value(nodes).unwrap();
                    BlockTask {
                        task_id,
                        chain,
                        chain_id,
                        start_height,
                        nodes,
                    }
                }).unwrap();
            Box::new(futures::future::ready(Ok(json!(tasks))))
        });
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
                    let message_map = message.as_object().unwrap();
                    let method = message_map.get("method").unwrap().as_str().unwrap().to_string();
                    let value = message_map.get("value").unwrap();
                    if method == String::from("insert_block_task") {
                        let block_task: BlockTask = serde_json::from_value(value.clone()).unwrap();
                        let mut conn = pool.lock().unwrap().get_conn().unwrap();
                        let _ = conn.exec_drop(
                            r"INSERT INTO ufc.block_task (task_id, chain, chain_id, start_height, nodes) VALUES (:task_id, :chain, :chain_id, :start_height, :nodes)",
                            params! {
                                "task_id" => block_task.task_id.clone(),
                                "chain" => block_task.chain.clone(),
                                "chain_id" => block_task.chain_id.clone(),
                                "start_height" => block_task.start_height,
                                "nodes" => block_task.nodes_str()
                            });
                    } else if method == String::from("delete_block_task") {
                        let task_id = value.as_str().unwrap().to_string();

                        let mut conn = pool.lock().unwrap().get_conn().unwrap();
                        let _ = conn.exec_drop(
                            r"DELETE FROM ufc.block_task WHERE task_id = :task_id",
                            params! {
                                "task_id" => task_id
                            });
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
