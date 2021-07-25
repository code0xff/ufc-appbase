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
use crate::types::block::{BlockTask, SubscribeBlock, SubscribeStatus};
use crate::validation::{subscribe, unsubscribe, get_block};

pub struct TendermintPlugin {
    base: PluginBase,
    sub_blocks: Option<SubscribeBlocks>,
    channels: Option<MultiChannel>,
    monitor: Option<SubscribeHandle>,
}

#[derive(Clone)]
struct MultiChannel {
    channel_map: HashMap<String, ChannelHandle>,
}

impl MultiChannel {
    fn new() -> MultiChannel {
        MultiChannel {
            channel_map: HashMap::new(),
        }
    }

    fn add(&mut self, name: &str) {
        self.channel_map.insert(String::from(name), app::get_channel(String::from(name)));
    }

    fn get(&self, name: &str) -> ChannelHandle {
        Arc::clone(self.channel_map.get(name).unwrap())
    }
}

type SubscribeBlocks = Arc<FutureMutex<HashMap<String, SubscribeBlock>>>;

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
    SubscribeBlock,
    UnsubscribeBlock,
}

impl TendermintMethod {
    fn value(&self) -> String {
        match self {
            TendermintMethod::SubscribeBlock => String::from("subscribe_block"),
            TendermintMethod::UnsubscribeBlock => String::from("unsubscribe_block"),
        }
    }

    fn find(method: &str) -> TendermintMethod {
        match method {
            "subscribe_block" => TendermintMethod::SubscribeBlock,
            "unsubscribe_block" => TendermintMethod::UnsubscribeBlock,
            _ => {
                panic!("matched method does not exist");
            }
        }
    }
}

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, RocksPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            sub_blocks: None,
            channels: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        self.sub_blocks = Some(Arc::new(FutureMutex::new(HashMap::new())));
        let mut channels = MultiChannel::new();
        channels.add("tendermint");
        channels.add("rocks");
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let rocks_channel = self.channels.as_ref().unwrap().get("rocks");
        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        jsonrpc.add_method(String::from("tm_subscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let message = TendermintMsg::new(TendermintMethod::SubscribeBlock, Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(message);

            let new_block_task = BlockTask::new(String::from("tendermint"), &params);
            let task_id = new_block_task.task_id.clone();
            let value = json!(new_block_task);
            let message = RocksMsg::new(RocksMethod::Put, task_id.clone(), Some(Value::String(value.to_string())));
            let _ = rocks_channel.lock().unwrap().send(message);

            Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
        });

        let rocks_channel = self.channels.as_ref().unwrap().get("rocks");
        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        jsonrpc.add_method(String::from("tm_unsubscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let tm_msg = TendermintMsg::new(TendermintMethod::UnsubscribeBlock, Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(tm_msg);

            let task_id = params.get("task_id").unwrap().as_str().unwrap().to_string();
            let rocks_msg = RocksMsg::new(RocksMethod::Delete, task_id.clone(), None);
            let _ = rocks_channel.lock().unwrap().send(rocks_msg);

            let task_id = params.get("task_id").unwrap().as_str().unwrap();
            Box::new(futures::future::ready(Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))))
        });

        let rocks_plugin_handle = app::get_plugin::<RocksPlugin>();
        let mut rocks_plugin = rocks_plugin_handle.lock().unwrap();
        let rocks = rocks_plugin.downcast_mut::<RocksPlugin>().unwrap();
        let raw_tasks: Value = rocks.find_by_prefix("task:block:tendermint");
        let sub_blocks = Arc::clone(self.sub_blocks.as_ref().unwrap());
        raw_tasks.as_array().unwrap().iter()
            .for_each(|val| {
                let task = val.as_object().unwrap();
                let block = SubscribeBlock::from(task);
                if block.is_workable() {
                    let mut sub_blocks_lock = sub_blocks.try_lock().unwrap();
                    sub_blocks_lock.insert(block.task_id.clone(), block);
                }
            });
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let sub_blocks = Arc::clone(self.sub_blocks.as_ref().unwrap());
        let rocks_channel = self.channels.as_ref().unwrap().get("rocks");
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                thread::sleep(Duration::from_secs(1));
                let mut sub_blocks_lock = sub_blocks.lock().await;
                if let Ok(message) = mon_lock.try_recv() {
                    let map = message.as_object().unwrap();
                    let method = TendermintMethod::find(map.get("method").unwrap().as_str().unwrap());
                    let params = map.get("value").unwrap().as_object().unwrap();
                    match method {
                        TendermintMethod::SubscribeBlock => {
                            let new_sub_block = SubscribeBlock::new(String::from("tendermint"), &params);
                            sub_blocks_lock.insert(new_sub_block.task_id.clone(), new_sub_block);
                        }
                        TendermintMethod::UnsubscribeBlock => {
                            let task_id = String::from(params.get("task_id").unwrap().as_str().unwrap());
                            sub_blocks_lock.remove(&task_id);
                        }
                    };
                }

                for (_, sub_block) in sub_blocks_lock.iter_mut() {
                    if sub_block.is_workable() {
                        let req_url = sub_block.request_url();
                        let res_result = reqwest::get(req_url).await;
                        if res_result.is_ok() {
                            let res = res_result.unwrap();
                            let status = res.status().clone();
                            let body = res
                                .text()
                                .await
                                .unwrap();
                            let map: Map<String, Value> = serde_json::from_str(body.as_str()).unwrap();
                            if status.is_success() {
                                let result = map.get("result").unwrap().as_object().unwrap();
                                let block = result.get("block").unwrap().as_object().unwrap();
                                let block_header = block.get("header").unwrap();

                                sub_block.curr_height += 1;
                                sub_block.status = SubscribeStatus::Working;

                                println!("{:?}", block_header);
                                // rabbit
                                // let _ = rabbit_channel.lock().unwrap().send(Value::String(block_header.to_string()));

                                // rocks
                                let msg = RocksMsg::new(RocksMethod::Put, sub_block.block_id(), Some(Value::String(block_header.to_string())));
                                let _ = rocks_channel.lock().unwrap().send(msg);
                            } else {
                                let err = map.get("error").unwrap().as_object().unwrap();
                                let err_code = err.get("code").unwrap().as_i64().unwrap();
                                let err_msg = err.get("data").unwrap().as_str().unwrap().to_string();
                                if status.is_server_error() {
                                    if err_code == -32603 {
                                        println!("waiting for next block...");
                                    } else {
                                        sub_block.handle_err(&rocks_channel, err_msg);
                                    }
                                } else if status == reqwest::StatusCode::NOT_FOUND {
                                    sub_block.handle_err(&rocks_channel, err_msg);
                                } else {
                                    sub_block.err(&rocks_channel, err_msg);
                                }
                            }
                        } else {
                            let err_msg = res_result.unwrap_err().to_string();
                            sub_block.handle_err(&rocks_channel, err_msg);
                        };
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
