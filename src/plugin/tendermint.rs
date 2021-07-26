use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::rocks::{RocksMethod, RocksMsg, RocksPlugin};
use crate::types::channel::MultiChannel;
use crate::types::jsonrpc::JsonRpcRequest;
use crate::types::subscribe::{SubscribeEvent, SubscribeTarget, SubscribeTask};
use crate::validation::{subscribe, unsubscribe};
use crate::types::enumeration::Enumeration;

pub struct TendermintPlugin {
    base: PluginBase,
    sub_events: Option<SubscribeEvents>,
    channels: Option<MultiChannel>,
    monitor: Option<SubscribeHandle>,
}

impl TendermintPlugin {
    fn init(&mut self) {
        self.sub_events = Some(Arc::new(FutureMutex::new(HashMap::new())));
        let channels = MultiChannel::new(vec!(String::from("tendermint"), String::from("rocks")));
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));
    }

    fn register_jsonrpc(&self) {
        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        jsonrpc.add_method(String::from("tm_subscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let message = TendermintMsg::new(TendermintMethod::Subscribe, Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(message);

            let task_id = SubscribeTask::task_id("tendermint", &params);
            Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
        });

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        jsonrpc.add_method(String::from("tm_unsubscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let tm_msg = TendermintMsg::new(TendermintMethod::Unsubscribe, Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(tm_msg);

            let task_id = params.get("task_id").unwrap().as_str().unwrap();
            Box::new(futures::future::ready(Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))))
        });
    }

    fn load_tasks(&self) {
        let rocks_plugin_handle = app::get_plugin::<RocksPlugin>();
        let mut rocks_plugin = rocks_plugin_handle.lock().unwrap();
        let rocks = rocks_plugin.downcast_mut::<RocksPlugin>().unwrap();
        let raw_tasks: Value = rocks.find_by_prefix("task:tendermint:");
        let sub_blocks = Arc::clone(self.sub_events.as_ref().unwrap());
        raw_tasks.as_array().unwrap().iter()
            .for_each(|val| {
                let task = val.as_object().unwrap();
                let block = SubscribeEvent::from(task);
                if block.is_workable() {
                    let mut sub_blocks_lock = sub_blocks.try_lock().unwrap();
                    sub_blocks_lock.insert(block.task_id.clone(), block);
                }
            });
    }
}

type SubscribeEvents = Arc<FutureMutex<HashMap<String, SubscribeEvent>>>;

#[derive(Serialize, Deserialize)]
pub struct TendermintMsg {
    method: String,
    value: Value,
}

impl TendermintMsg {
    pub fn new(method: TendermintMethod, value: Value) -> Value {
        let msg = TendermintMsg {
            method: method.value(),
            value,
        };
        json!(msg)
    }
}

pub enum TendermintMethod {
    Subscribe,
    Unsubscribe,
}

impl Enumeration for TendermintMethod {
    fn value(&self) -> String {
        match self {
            TendermintMethod::Subscribe => String::from("subscribe"),
            TendermintMethod::Unsubscribe => String::from("unsubscribe"),
        }
    }

    fn find(method: &str) -> Option<Self> {
        match method {
            "subscribe" => Some(TendermintMethod::Subscribe),
            "unsubscribe" => Some(TendermintMethod::Unsubscribe),
            _ => None
        }
    }
}

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, RocksPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            sub_events: None,
            channels: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        self.init();
        self.register_jsonrpc();
        self.load_tasks();
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let sub_events = Arc::clone(self.sub_events.as_ref().unwrap());
        let rocks_channel = self.channels.as_ref().unwrap().get("rocks");
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                thread::sleep(Duration::from_secs(1));
                let mut sub_events_lock = sub_events.lock().await;
                if let Ok(message) = mon_lock.try_recv() {
                    let map = message.as_object().unwrap();
                    let method = TendermintMethod::find(map.get("method").unwrap().as_str().unwrap()).unwrap();
                    let params = map.get("value").unwrap().as_object().unwrap();
                    match method {
                        TendermintMethod::Subscribe => {
                            let new_event = SubscribeEvent::new(String::from("tendermint"), &params);
                            sub_events_lock.insert(new_event.task_id.clone(), new_event.clone());

                            let new_task = SubscribeTask::from(&new_event);
                            let task_id = new_task.task_id.clone();
                            let value = json!(new_task);
                            let message = RocksMsg::new(RocksMethod::Put, task_id.clone(), Some(Value::String(value.to_string())));
                            let _ = rocks_channel.lock().unwrap().send(message);
                        }
                        TendermintMethod::Unsubscribe => {
                            let task_id = String::from(params.get("task_id").unwrap().as_str().unwrap());
                            sub_events_lock.remove(&task_id);

                            let rocks_msg = RocksMsg::new(RocksMethod::Delete, task_id.clone(), None);
                            let _ = rocks_channel.lock().unwrap().send(rocks_msg);
                        }
                    };
                }

                for (_, sub_event) in sub_events_lock.iter_mut() {
                    if sub_event.is_workable() {
                        let res_result = match sub_event.target {
                            SubscribeTarget::Block => {
                                let node_index = usize::from(sub_event.node_idx);
                                let req_url = sub_event.nodes[node_index].to_string() + sub_event.curr_height.to_string().as_str();
                                reqwest::get(req_url).await
                            }
                            SubscribeTarget::Tx => {
                                let req_body = JsonRpcRequest::params(sub_event.curr_height);
                                let req = JsonRpcRequest::new(1, String::from("tx_search"), req_body);
                                let client = reqwest::Client::new();
                                let node_index = usize::from(sub_event.node_idx);
                                client.post(sub_event.nodes[node_index].to_string())
                                    .body(json!(req).to_string())
                                    .send()
                                    .await
                            }
                        };

                        match res_result {
                            Ok(res) => {
                                let status = res.status().clone();
                                let body = res
                                    .text()
                                    .await
                                    .unwrap();
                                let map: Map<String, Value> = serde_json::from_str(body.as_str()).unwrap();
                                if status.is_success() {
                                    match sub_event.target {
                                        SubscribeTarget::Block => {
                                            let result = map.get("result").unwrap().as_object().unwrap();
                                            let block = result.get("block").unwrap().as_object().unwrap();
                                            let header = block.get("header").unwrap();

                                            println!("event_id={}, header={:?}", sub_event.event_id(), header);
                                        }
                                        SubscribeTarget::Tx => {
                                            let result = map.get("result").unwrap().as_object().unwrap();
                                            let txs = result.get("txs").unwrap().as_array().unwrap();
                                            for tx_val in txs.iter() {
                                                let tx = tx_val.as_object().unwrap();
                                                let hash = tx.get("hash").unwrap().as_str().unwrap();
                                                let height = tx.get("height").unwrap().as_str().unwrap();
                                                let tx_result = tx.get("tx_result").unwrap().as_object().unwrap();
                                                let log = tx_result.get("log").unwrap().as_str().unwrap();

                                                println!("event_id={}, hash={}, height={}, log={}", sub_event.event_id(), hash, height, log);
                                            }
                                        }
                                    }

                                    let task = SubscribeTask::from(&sub_event);
                                    let task_id = task.task_id.clone();
                                    let value = json!(task);
                                    let message = RocksMsg::new(RocksMethod::Put, task_id.clone(), Some(Value::String(value.to_string())));
                                    let _ = rocks_channel.lock().unwrap().send(message);

                                    sub_event.curr_height += 1;
                                } else {
                                    let err = map.get("error").unwrap().as_object().unwrap();
                                    let err_code = err.get("code").unwrap().as_i64().unwrap();
                                    let err_msg = err.get("data").unwrap().as_str().unwrap().to_string();
                                    if status.is_server_error() {
                                        if err_code == -32603 {
                                            println!("waiting for next block...");
                                        } else {
                                            sub_event.handle_err(&rocks_channel, err_msg);
                                        }
                                    } else if status == reqwest::StatusCode::NOT_FOUND {
                                        sub_event.handle_err(&rocks_channel, err_msg);
                                    } else {
                                        sub_event.err(&rocks_channel, err_msg);
                                    }
                                }
                            }
                            Err(err) => {
                                sub_event.handle_err(&rocks_channel, err.to_string());
                            }
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
