use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::rabbit::RabbitPlugin;
use crate::plugin::rocks::RocksPlugin;
use crate::types::block::{BlockTask, SubscribeBlock, SubscribeStatus};
use crate::validation::{get_task, subscribe, unsubscribe};

pub struct TendermintPlugin {
    base: PluginBase,
    block_tasks: Option<BlockTasks>,
    subscribe_blocks: Option<SubscribeBlocks>,
    channels: Option<HashMap<String, ChannelHandle>>,
    monitor: Option<SubscribeHandle>,
}

type BlockTasks = Arc<Mutex<HashMap<String, BlockTask>>>;
type SubscribeBlocks = Arc<FutureMutex<HashMap<String, SubscribeBlock>>>;

impl TendermintPlugin {
    pub fn gen_msg(method: String, value: Map<String, Value>) -> Value {
        let mut message = Map::new();
        message.insert(String::from("method"), Value::String(method));
        message.insert(String::from("params"), Value::Object(value));
        Value::Object(message)
    }
}

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, RabbitPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            block_tasks: None,
            subscribe_blocks: None,
            channels: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        self.block_tasks = Some(Arc::new(Mutex::new(HashMap::new())));
        self.subscribe_blocks = Some(Arc::new(FutureMutex::new(HashMap::new())));
        let mut channels: HashMap<String, ChannelHandle> = HashMap::new();
        channels.insert(String::from("tendermint"), app::get_channel(String::from("tendermint")));
        channels.insert(String::from("rocks"), app::get_channel(String::from("rocks")));
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();
        let tendermint_channel = Arc::clone(self.channels.as_ref().unwrap().get("tendermint").unwrap());
        let block_tasks = Arc::clone(self.block_tasks.as_ref().unwrap());
        jsonrpc.add_method(String::from("tm_subscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let message = Self::gen_msg(String::from("subscribe_block"), params.clone());
            let _ = tendermint_channel.lock().unwrap().send(message);

            let task_id = format!("{}:{}", "tendermint", params.get("chain_id").unwrap().as_str().unwrap());
            let new_block_task = BlockTask::new(String::from("tendermint"), &params);
            block_tasks.lock().unwrap().insert(task_id.clone(), new_block_task);

            Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
        });

        let block_tasks = Arc::clone(self.block_tasks.as_ref().unwrap());
        let tendermint_channel = Arc::clone(self.channels.as_ref().unwrap().get("tendermint").unwrap());
        jsonrpc.add_method(String::from("tm_unsubscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let message = Self::gen_msg(String::from("unsubscribe_block"), params.clone());
            let _ = tendermint_channel.lock().unwrap().send(message);

            let task_id = params.get("task_id").unwrap().as_str().unwrap();
            block_tasks.lock().unwrap().remove(task_id.clone());

            Box::new(futures::future::ready(Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))))
        });

        let block_tasks = Arc::clone(self.block_tasks.as_ref().unwrap());
        jsonrpc.add_method(String::from("tm_get_block_task"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_task::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            Box::new(futures::future::ready(Ok(Value::String(format!("{:?}", block_tasks.lock().unwrap())))))
        });
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        let rocks_channel = Arc::clone(self.channels.as_ref().unwrap().get("rocks").unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                thread::sleep(Duration::from_secs(1));
                let mut locked_subscribe_blocks = subscribe_blocks.lock().await;
                if let Ok(message) = locked_monitor.try_recv() {
                    let message_map = message.as_object().unwrap();
                    let method = String::from(message_map.get("method").unwrap().as_str().unwrap());
                    let params = message_map.get("params").unwrap().as_object().unwrap();
                    if method == String::from("subscribe_block") {
                        let new_subscribe_block = SubscribeBlock::new(String::from("tendermint"), &params);
                        locked_subscribe_blocks.insert(format!("{}:{}", String::from("tendermint"), new_subscribe_block.chain_id), new_subscribe_block);
                    } else if method == String::from("unsubscribe_block") {
                        let chain = String::from(params.get("task_id").unwrap().as_str().unwrap());
                        locked_subscribe_blocks.remove(&chain);
                    }
                }

                for (_, subscribe_block) in locked_subscribe_blocks.iter_mut() {
                    if subscribe_block.is_workable() {
                        let request_url = subscribe_block.request_url();
                        let response = reqwest::get(request_url)
                            .await
                            .unwrap();

                        if response.status().is_client_error() {
                            if response.status().as_u16() == 404 && usize::from(subscribe_block.node_index) + 1 < subscribe_block.nodes.len() {
                                subscribe_block.node_index += 1;
                            } else {
                                subscribe_block.status = SubscribeStatus::Error;
                            }
                        } else {
                            let body = response
                                .text()
                                .await
                                .unwrap();

                            let body_map: Map<String, Value> = serde_json::from_str(body.as_str()).unwrap();
                            if body_map.get("error").is_none() {
                                let result = body_map.get("result").unwrap().as_object().unwrap();
                                let block = result.get("block").unwrap().as_object().unwrap();
                                let block_header = block.get("header").unwrap();

                                subscribe_block.current_height += 1;
                                subscribe_block.status = SubscribeStatus::Working;

                                println!("{:?}", block_header);
                                // rabbit
                                // let _ = rabbit_channel.lock().unwrap().send(block_header.clone());

                                // rocks
                                let put_message = RocksPlugin::gen_msg(String::from("put"), format!("{}:{}:{}", subscribe_block.chain, subscribe_block.chain_id, subscribe_block.current_height), block_header.clone());
                                let _ = rocks_channel.lock().unwrap().send(put_message);
                            } else {
                                println!("{:?}", body_map.get("error").unwrap());
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
