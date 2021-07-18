use std::convert::TryFrom;
use std::sync::Arc;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mongo::MongoPlugin;
use crate::plugin::monitor::MonitorPlugin;
use crate::plugin::rabbit::RabbitPlugin;
use crate::plugin::rocks::RocksPlugin;
use crate::types::block::{SubscribeBlock, SubscribeStatus};

pub struct TendermintPlugin {
    base: PluginBase,
    subscribe_blocks: Option<SubscribeBlocks>,
    channel: Option<ChannelHandle>,
    monitor: Option<SubscribeHandle>,
}

type SubscribeBlocks = Arc<FutureMutex<Vec<SubscribeBlock>>>;

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, MonitorPlugin, RabbitPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            subscribe_blocks: None,
            channel: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        unsafe {
            self.subscribe_blocks = Some(Arc::new(FutureMutex::new(Vec::new())));
            self.channel = Some(APP.get_channel(String::from("rabbit")));
            self.monitor = Some(APP.subscribe_channel(String::from("tendermint")));
        }
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }

        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let channel = Arc::clone(self.channel.as_ref().unwrap());
        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                if let Ok(message) = locked_monitor.try_recv() {
                    let block = message.as_object().unwrap();
                    let block_nodes = block.get("nodes").unwrap().as_array().unwrap();
                    let start_height = block.get("start_height").unwrap().as_u64().unwrap();
                    let mut nodes: Vec<String> = Vec::new();
                    for n in block_nodes.iter() {
                        nodes.push(String::from(n.as_str().unwrap()));
                    }
                    let new_subscribe_block = SubscribeBlock {
                        chain: String::from(block.get("chain").unwrap().as_str().unwrap()),
                        chain_id: String::from(block.get("chain_id").unwrap().as_str().unwrap()),
                        start_height,
                        current_height: start_height,
                        nodes,
                        node_index: 0,
                    };
                    let mut locked_subscribe_blocks = subscribe_blocks.lock().await;
                    locked_subscribe_blocks.push(new_subscribe_block);
                }

                let mut locked_subscribe_blocks = subscribe_blocks.lock().await;
                for subscribe_block in locked_subscribe_blocks.iter_mut() {
                    if subscribe_block.status == SubscribeStatus::Requested && subscribe_block.status == SubscribeStatus::Working {
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
                                .await;

                            let body_map: Map<String, Value> = serde_json::from_str(body.unwrap().as_str()).unwrap();
                            if body_map.get("error").is_none() {
                                let result = body_map.get("result").unwrap().as_object().unwrap();
                                let block = result.get("block").unwrap().as_object().unwrap();
                                let block_header = block.get("header").unwrap();

                                subscribe_block.current_height += 1;
                                subscribe_block.status = SubscribeStatus::Working;

                                //rabbit
                                let _ = channel.lock().unwrap().send(block_header.clone());

                                // mongo
                                // let mut data = Map::new();
                                // data.insert(String::from("collection"), Value::String(String::from("block")));
                                // data.insert(String::from("document"), block_header.clone());
                                // let _ = channel.lock().unwrap().send(Value::Object(data));

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
