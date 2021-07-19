use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::{Arc};

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mongo::MongoPlugin;
use crate::plugin::rabbit::RabbitPlugin;
use crate::plugin::rocks::RocksPlugin;
use crate::types::block::{SubscribeBlock, SubscribeStatus};
use crate::validation::{get_task, subscribe, unsubscribe};

pub struct TendermintPlugin {
    base: PluginBase,
    subscribe_blocks: Option<SubscribeBlocks>,
    channels: Option<HashMap<String, ChannelHandle>>,
    monitor: Option<SubscribeHandle>,
}

type SubscribeBlocks = Arc<FutureMutex<HashMap<String, SubscribeBlock>>>;

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, RabbitPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            subscribe_blocks: None,
            channels: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        self.subscribe_blocks = Some(Arc::new(FutureMutex::new(HashMap::new())));
        let mut channels: HashMap<String, ChannelHandle> = HashMap::new();
        channels.insert(String::from("tendermint"), app::get_channel(String::from("tendermint")));
        channels.insert(String::from("rabbit"), app::get_channel(String::from("rabbit")));
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));

        let jsonrpc_plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut jsonrpc_plugin = jsonrpc_plugin_handle.lock().unwrap();
        let jsonrpc = jsonrpc_plugin.downcast_mut::<JsonRpcPlugin>().unwrap();
        let tendermint_channel = Arc::clone(self.channels.as_ref().unwrap().get("tendermint").unwrap());
        jsonrpc.add_method(String::from("tm_subscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let mut message = Map::new();
            message.insert(String::from("method"), Value::String(String::from("subscribe_block")));
            message.insert(String::from("params"), Value::Object(params.clone()));
            let _ = tendermint_channel.lock().unwrap().send(Value::Object(message));

            let task_id = format!("{}:{}", "tendermint", params.get("chain_id").unwrap().as_str().unwrap());
            Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
        });

        let tendermint_channel = Arc::clone(self.channels.as_ref().unwrap().get("tendermint").unwrap());
        jsonrpc.add_method(String::from("tm_unsubscribe_block"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let mut message = Map::new();
            message.insert(String::from("method"), Value::String(String::from("unsubscribe_block")));
            message.insert(String::from("params"), Value::Object(params.clone()));
            let _ = tendermint_channel.lock().unwrap().send(Value::Object(message));

            let task_id = params.get("task_id").unwrap().as_str().unwrap();
            Box::new(futures::future::ready(Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))))
        });

        jsonrpc.add_method(String::from("tm_get_block_task"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_task::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            Box::new(futures::future::ready(Ok(Value::String("test".to_string()))))
        });
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        let rabbit_channel = Arc::clone(self.channels.as_ref().unwrap().get("rabbit").unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                let mut locked_subscribe_blocks = subscribe_blocks.lock().await;
                if let Ok(message) = locked_monitor.try_recv() {
                    let message_map = message.as_object().unwrap();
                    let method = String::from(message_map.get("method").unwrap().as_str().unwrap());
                    let params = message_map.get("params").unwrap().as_object().unwrap();
                    if method == String::from("subscribe_block") {
                        let chain = String::from("tendermint");
                        let chain_id = String::from(params.get("chain_id").unwrap().as_str().unwrap());
                        let block_nodes = params.get("nodes").unwrap().as_array().unwrap();
                        let start_height = params.get("start_height").unwrap().as_u64().unwrap();
                        let mut nodes: Vec<String> = Vec::new();
                        for n in block_nodes.iter() {
                            nodes.push(String::from(n.as_str().unwrap()));
                        }
                        let new_subscribe_block = SubscribeBlock {
                            chain: chain.clone(),
                            chain_id: chain_id.clone(),
                            start_height,
                            current_height: start_height,
                            nodes,
                            node_index: 0,
                            status: SubscribeStatus::Requested,
                        };

                        locked_subscribe_blocks.insert(format!("{}:{}", chain.clone(), chain_id.clone()), new_subscribe_block);
                    } else if method == String::from("unsubscribe_block") {
                        let chain = String::from(params.get("task_id").unwrap().as_str().unwrap());
                        locked_subscribe_blocks.remove(&chain);
                    }
                }

                for (_, subscribe_block) in locked_subscribe_blocks.iter_mut() {
                    if subscribe_block.status == SubscribeStatus::Requested || subscribe_block.status == SubscribeStatus::Working {
                        let node_index = usize::try_from(subscribe_block.node_index).unwrap();
                        let node_url = subscribe_block.nodes[node_index].as_str().to_owned() + subscribe_block.current_height.to_string().as_str();

                        let response = reqwest::get(node_url)
                            .await
                            .unwrap();

                        if response.status().is_client_error() {
                            subscribe_block.status = SubscribeStatus::RequestError;
                        } else if response.status().is_server_error() {
                            if usize::from(subscribe_block.node_index) + 1 < subscribe_block.nodes.len() {
                                subscribe_block.node_index += 1;
                            } else {
                                subscribe_block.status = SubscribeStatus::ServerError;
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

                                // println!("{:?}", block_header);

                                // rabbit
                                let _ = rabbit_channel.lock().unwrap().send(block_header.clone());

                                // mongo
                                // let mut data = Map::new();
                                // data.insert(String::from("collection"), Value::String(String::from("block")));
                                // data.insert(String::from("document"), block_header.clone());
                                // let _ = mongo_channel.lock().unwrap().send(Value::Object(data));

                                // rocks
                                // let mut data = Map::new();
                                // let key = format!("{}:{}:{}", subscribe_block.chain, subscribe_block.chain_id, subscribe_block.current_height);
                                // data.insert(String::from("key"), Value::String(key));
                                // data.insert(String::from("value"), Value::String(block_header.to_string()));
                                // let _ = channel.lock().unwrap().send(Value::Object(data));
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
