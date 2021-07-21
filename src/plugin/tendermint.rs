use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde_json::{json, Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::rabbit::RabbitPlugin;
use crate::plugin::rocks::RocksPlugin;
use crate::types::block::{BlockTask, SubscribeBlock, SubscribeStatus};
use crate::validation::{subscribe, unsubscribe};

pub struct TendermintPlugin {
    base: PluginBase,
    sub_blocks: Option<SubscribeBlocks>,
    channels: Option<HashMap<String, ChannelHandle>>,
    monitor: Option<SubscribeHandle>,
}

type SubscribeBlocks = Arc<FutureMutex<HashMap<String, SubscribeBlock>>>;

impl TendermintPlugin {
    pub fn gen_msg(method: String, val: Value) -> Value {
        let mut msg = Map::new();
        msg.insert(String::from("method"), Value::String(method));
        msg.insert(String::from("params"), val);
        Value::Object(msg)
    }
}

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, RabbitPlugin, RocksPlugin);

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
        let mut channels: HashMap<String, ChannelHandle> = HashMap::new();
        channels.insert(String::from("tendermint"), app::get_channel(String::from("tendermint")));
        channels.insert(String::from("rocks"), app::get_channel(String::from("rocks")));
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let rocks_channel = Arc::clone(self.channels.as_ref().unwrap().get("rocks").unwrap());
        let tm_channel = Arc::clone(self.channels.as_ref().unwrap().get("tendermint").unwrap());
        jsonrpc.add_method(String::from("tm_subscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let message = Self::gen_msg(String::from("subscribe_block"), Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(message);

            let new_block_task = BlockTask::new(String::from("tendermint"), &params);
            let task_id = new_block_task.task_id.clone();
            let value = json!(new_block_task);
            let message = RocksPlugin::gen_msg(String::from("put"), task_id.clone(), Some(Value::String(value.to_string())));
            let _ = rocks_channel.lock().unwrap().send(message);

            Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
        });

        let rocks_channel = Arc::clone(self.channels.as_ref().unwrap().get("rocks").unwrap());
        let tm_channel = Arc::clone(self.channels.as_ref().unwrap().get("tendermint").unwrap());
        jsonrpc.add_method(String::from("tm_unsubscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let tm_msg = Self::gen_msg(String::from("unsubscribe_block"), Value::Object(params.clone()));
            let _ = tm_channel.lock().unwrap().send(tm_msg);

            let task_id = params.get("task_id").unwrap().as_str().unwrap().to_string();
            let rocks_msg = RocksPlugin::gen_msg(String::from("delete"), task_id.clone(), None);
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
        let rocks_ch = Arc::clone(self.channels.as_ref().unwrap().get("rocks").unwrap());
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                thread::sleep(Duration::from_secs(1));
                let mut sub_blocks_lock = sub_blocks.lock().await;
                if let Ok(message) = mon_lock.try_recv() {
                    let map = message.as_object().unwrap();
                    let method = String::from(map.get("method").unwrap().as_str().unwrap());
                    let params = map.get("params").unwrap().as_object().unwrap();
                    if method == String::from("subscribe_block") {
                        let new_sub_block = SubscribeBlock::new(String::from("tendermint"), &params);
                        sub_blocks_lock.insert(new_sub_block.task_id.clone(), new_sub_block);
                    } else if method == String::from("unsubscribe_block") {
                        let chain = String::from(params.get("task_id").unwrap().as_str().unwrap());
                        sub_blocks_lock.remove(&chain);
                    }
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
                                let msg = RocksPlugin::gen_msg(String::from("put"), sub_block.block_id(), Some(Value::String(block_header.to_string())));
                                let _ = rocks_ch.lock().unwrap().send(msg);
                            } else {
                                let err = map.get("error").unwrap().as_object().unwrap();
                                let err_code = err.get("code").unwrap().as_i64().unwrap();
                                let err_msg = err.get("data").unwrap().as_str().unwrap().to_string();
                                println!("{}", err_msg);
                                if status.is_server_error() {
                                    if err_code == -32603 {
                                        println!("waiting for next block...");
                                    } else {
                                        sub_block.handle_err(&rocks_ch, err_msg);
                                    }
                                } else if status == reqwest::StatusCode::NOT_FOUND {
                                    sub_block.handle_err(&rocks_ch, err_msg);
                                } else {
                                    sub_block.err(&rocks_ch, err_msg);
                                }
                            }
                        } else {
                            let err_msg = res_result.unwrap_err().to_string();
                            println!("{}", err_msg);
                            sub_block.handle_err(&rocks_ch, err_msg);
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
