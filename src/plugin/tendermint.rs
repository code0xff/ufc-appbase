use std::{env, thread};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use appbase::*;
use dotenv::dotenv;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{enumeration, message};
use crate::libs::mysql::query;
use crate::libs::rocks::{get_by_prefix_static, get_static};
use crate::libs::serde::{get_object, get_str, get_string, pick};
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mysql::{MySqlMethod, MySqlMsg, MySqlPlugin};
use crate::plugin::rabbit::RabbitPlugin;
use crate::plugin::rocks::{RocksMethod, RocksMsg, RocksPlugin};
use crate::types::channel::MultiChannel;
use crate::types::enumeration::Enumeration;
use crate::types::subscribe::{SubscribeEvent, SubscribeTarget, SubscribeTask};
use crate::validation::{get_task, subscribe, unsubscribe};

pub struct TendermintPlugin {
    base: PluginBase,
    sub_events: Option<SubscribeEvents>,
    channels: Option<MultiChannel>,
    monitor: Option<SubscribeHandle>,
}

const CHAIN: &str = "tendermint";
const TASK_PREFIX: &str = "task:tendermint";

impl TendermintPlugin {
    fn init(&mut self) {
        self.sub_events = Some(Arc::new(FutureMutex::new(HashMap::new())));
        let channels = MultiChannel::new(vec!(String::from("tendermint"), String::from("rocks"), String::from("mysql"), String::from("rabbit")));
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));
    }

    fn register_jsonrpc(&self) {
        let plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let jsonrpc = plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let plugin_handle = app::get_plugin::<RocksPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let rocks = plugin.downcast_mut::<RocksPlugin>().unwrap();

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_subscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let message = TendermintMsg::new(TendermintMethod::Subscribe, Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(message);
            let task_id = SubscribeTask::task_id(CHAIN, &params);

            let value = get_static(&rocks_db, task_id.as_str());
            if value.is_null() {
                let task_id = SubscribeTask::task_id(CHAIN, &params);
                Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
            } else {
                Box::new(futures::future::ready(Ok(Value::String(format!("already exist task! task_id={}", task_id)))))
            }
        });


        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_unsubscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let tm_msg = TendermintMsg::new(TendermintMethod::Unsubscribe, Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(tm_msg);

            let task_id = get_str(&params, "task_id").unwrap();
            let value = get_static(&rocks_db, task_id);
            if value.is_null() {
                Box::new(futures::future::ready(Ok(Value::String(format!("task does not exist! task_id={}", task_id)))))
            } else {
                Box::new(futures::future::ready(Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))))
            }
        });

        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_get_tasks"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_task::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let prefix = match params.get("task_id") {
                None => {
                    TASK_PREFIX
                }
                Some(task_id) => {
                    task_id.as_str().unwrap()
                }
            };
            let tasks = get_by_prefix_static(&rocks_db, prefix);
            Box::new(futures::future::ready(Ok(tasks)))
        });
    }

    fn load_tasks(&self) {
        let plugin_handle = app::get_plugin::<RocksPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let rocks = plugin.downcast_mut::<RocksPlugin>().unwrap();

        let rocks_db = rocks.get_db();
        let raw_tasks = get_by_prefix_static(&rocks_db, TASK_PREFIX);
        let sub_events = Arc::clone(self.sub_events.as_ref().unwrap());
        raw_tasks.as_array().unwrap().iter()
            .for_each(|raw_task| {
                let task = raw_task.as_object().unwrap();
                let event = SubscribeEvent::from(task);
                if event.is_workable() {
                    let mut sub_events_lock = sub_events.try_lock().unwrap();
                    sub_events_lock.insert(event.task_id.clone(), event);
                }
            });
    }

    fn sync_event(rocks_channel: &ChannelHandle, sub_event: &mut SubscribeEvent) {
        let task = SubscribeTask::from(&sub_event, String::from(""));
        let task_id = task.task_id.clone();

        let msg = RocksMsg::new(RocksMethod::Put, task_id, Value::String(json!(task).to_string()));
        let _ = rocks_channel.lock().unwrap().send(msg);
    }

    fn error_handler(rocks_channel: &ChannelHandle, sub_event: &mut SubscribeEvent, err_msg: String) {
        sub_event.handle_err(err_msg.clone());
        let task = SubscribeTask::from(sub_event, err_msg);

        let msg = RocksMsg::new(RocksMethod::Put, sub_event.task_id.clone(), Value::String(json!(task).to_string()));
        let _ = rocks_channel.lock().unwrap().send(msg);
    }
}

type SubscribeEvents = Arc<FutureMutex<HashMap<String, SubscribeEvent>>>;

message!((TendermintMsg; {value: Value}); (TendermintMethod; {Subscribe: "subscribe"}, {Unsubscribe: "unsubscribe"}));

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
        let mysql_channel = self.channels.as_ref().unwrap().get("mysql");
        let rabbit_channel = self.channels.as_ref().unwrap().get("rabbit");
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                thread::sleep(Duration::from_secs(1));
                let mut sub_events_lock = sub_events.lock().await;
                if let Ok(message) = mon_lock.try_recv() {
                    let message_map = message.as_object().unwrap();
                    let method = TendermintMethod::find(get_str(message_map, "method").unwrap()).unwrap();
                    let params = get_object(message_map, "value").unwrap();
                    match method {
                        TendermintMethod::Subscribe => {
                            let new_event = SubscribeEvent::new(CHAIN, &params);
                            sub_events_lock.insert(new_event.task_id.clone(), new_event.clone());

                            let task = SubscribeTask::from(&new_event, String::from(""));
                            let msg = RocksMsg::new(RocksMethod::Put, new_event.task_id, Value::String(json!(task).to_string()));
                            let _ = rocks_channel.lock().unwrap().send(msg);
                        }
                        TendermintMethod::Unsubscribe => {
                            let task_id = get_string(&params, "task_id").unwrap();
                            sub_events_lock.remove(&task_id);

                            let msg = RocksMsg::new(RocksMethod::Delete, task_id, Value::Null);
                            let _ = rocks_channel.lock().unwrap().send(msg);
                        }
                    };
                }

                for (_, sub_event) in sub_events_lock.iter_mut() {
                    if sub_event.is_workable() {
                        let res_result = match sub_event.target {
                            SubscribeTarget::Block => {
                                let node_index = usize::from(sub_event.node_idx);
                                let req_url = format!("{}/blocks/{}", sub_event.nodes[node_index], sub_event.curr_height);
                                reqwest::get(req_url).await
                            }
                            SubscribeTarget::Tx => {
                                let node_idx = usize::from(sub_event.node_idx);
                                let latest_req_url = format!("{}/blocks/latest", sub_event.nodes[node_idx]);
                                let res_result = reqwest::get(latest_req_url).await;
                                if res_result.is_ok() {
                                    let res_body = res_result.unwrap().text().await;
                                    if res_body.is_ok() {
                                        let body: Map<String, Value> = serde_json::from_str(res_body.unwrap().as_str()).unwrap();
                                        let block = get_object(&body, "block");
                                        if block.is_err() {
                                            println!("{}", block.unwrap_err());
                                            continue;
                                        }
                                        let unwrapped_block = block.unwrap();
                                        let header = get_object(&unwrapped_block, "header");
                                        if header.is_err() {
                                            println!("{}", header.unwrap_err());
                                            continue;
                                        }
                                        let unwrapped_header = header.unwrap();
                                        let height = get_str(&unwrapped_header, "height");
                                        if height.is_err() {
                                            println!("{}", height.unwrap_err());
                                            continue;
                                        }
                                        let latest_height = u64::from_str(height.unwrap()).unwrap();
                                        if sub_event.curr_height <= latest_height {
                                            let node_index = usize::from(sub_event.node_idx);
                                            let req_url = format!("{}/txs?page=1&limit=100&tx.height={}", sub_event.nodes[node_index], sub_event.curr_height);
                                            reqwest::get(req_url).await
                                        } else {
                                            println!("waiting for next block...");
                                            continue;
                                        }
                                    } else {
                                        Self::error_handler(&rocks_channel, sub_event, res_body.unwrap_err().to_string());
                                        continue;
                                    }
                                } else {
                                    Self::error_handler(&rocks_channel, sub_event, res_result.unwrap_err().to_string());
                                    continue;
                                }
                            }
                        };

                        match res_result {
                            Ok(res) => {
                                let status = res.status().clone();
                                let res_body = res.text().await;
                                match res_body {
                                    Ok(body_str) => {
                                        let parsed_body = serde_json::from_str(body_str.as_str());
                                        if parsed_body.is_err() {
                                            println!("parsing error occurred...");
                                            continue;
                                        }
                                        let body: Map<String, Value> = parsed_body.unwrap();
                                        if status.is_success() {
                                            match sub_event.target {
                                                SubscribeTarget::Block => {
                                                    let block = get_object(&body, "block");
                                                    if block.is_err() {
                                                        println!("{}", block.unwrap_err());
                                                        continue;
                                                    }
                                                    let header = block.unwrap().get("header").unwrap();

                                                    println!("event_id={}, header={}", sub_event.event_id(), header.to_string());

                                                    dotenv().ok();
                                                    if bool::from_str(env::var("TM_BLOCK_MYSQL_SYNC").unwrap().as_str()).unwrap() {}
                                                    if bool::from_str(env::var("TM_BLOCK_RABBIT_PUBLISH").unwrap().as_str()).unwrap() {
                                                        let _ = rabbit_channel.lock().unwrap().send(Value::String(header.to_string()));
                                                    }
                                                }
                                                SubscribeTarget::Tx => {
                                                    let txs = body.get("txs").unwrap().as_array().unwrap();
                                                    for tx in txs.iter() {
                                                        println!("event_id={}, tx={}", sub_event.event_id(), tx.to_string());

                                                        dotenv().ok();
                                                        if bool::from_str(env::var("TM_TX_MYSQL_SYNC").unwrap().as_str()).unwrap() {
                                                            let tx_object = tx.as_object().unwrap();
                                                            let query = query("tm_tx", vec!["txhash", "height", "gas_wanted", "gas_used", "raw_log", "timestamp"]);
                                                            let values = pick(tx_object, vec!["txhash", "height", "gas_wanted", "gas_used", "raw_log", "timestamp"]);
                                                            if values.is_ok() {
                                                                let mysql_msg = MySqlMsg::new(MySqlMethod::Insert, String::from(query), Value::Object(values.unwrap()));
                                                                let _ = mysql_channel.lock().unwrap().send(mysql_msg);
                                                            } else {
                                                                println!("{}", values.unwrap_err());
                                                            }
                                                        }
                                                        if bool::from_str(env::var("TM_TX_RABBIT_PUBLISH").unwrap().as_str()).unwrap() {
                                                            let _ = rabbit_channel.lock().unwrap().send(Value::String(tx.to_string()));
                                                        }
                                                    }
                                                }
                                            }
                                            Self::sync_event(&rocks_channel, sub_event);

                                            sub_event.curr_height += 1;
                                        } else {
                                            let err_msg = get_string(&body, "error").unwrap();
                                            if err_msg.starts_with("requested block height") {
                                                println!("waiting for next block...");
                                            } else {
                                                Self::error_handler(&rocks_channel, sub_event, err_msg);
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                    }
                                };
                            }
                            Err(err) => {
                                Self::error_handler(&rocks_channel, sub_event, err.to_string());
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
