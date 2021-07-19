use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::{Params, BoxFuture};
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mongo::MongoPlugin;
use crate::plugin::rabbit::RabbitPlugin;
use crate::plugin::rocks::RocksPlugin;
use crate::types::block::{SubscribeBlock, SubscribeStatus};
use crate::validation::subscribe;

pub struct TendermintPlugin {
    base: PluginBase,
    subscribe_blocks: Option<SubscribeBlocks>,
    channel: Option<ChannelHandle>,
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
            channel: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        unsafe {
            self.subscribe_blocks = Some(Arc::new(FutureMutex::new(HashMap::new())));
            self.channel = Some(APP.get_channel(String::from("rabbit")));
        }

        let plugin_handle: PluginHandle;
        unsafe {
            plugin_handle = APP.get_plugin::<JsonRpcPlugin>();
        }
        let mut plugin = plugin_handle.lock().unwrap();
        let jsonrpc = plugin.downcast_mut::<JsonRpcPlugin>().unwrap();
        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        jsonrpc.add_method(String::from("tm_subscribe_block"), move |params: Params| async {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Ok(Value::String(verified.unwrap_err()));
            }
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
            let mut locked_subscribe_blocks = subscribe_blocks.lock().await;
            let task_id = format!("{}:{}", chain, chain_id);
            locked_subscribe_blocks.insert(task_id.clone(), new_subscribe_block);

            futures::future::ready(Ok(Value::String(format!("subscription requested!"))))
        });

        // jsonrpc.add_sync_method(String::from("tm_unsubscribe_block"), move |params: Params| {
        //     let params: Map<String, Value> = params.parse().unwrap();
        //     let task_id = params.get("task_id").unwrap().as_str().unwrap().to_string();
        //
        //     locked_subscribe_blocks.remove(&task_id);
        //
        //     Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))
        // });
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }

        let channel = Arc::clone(self.channel.as_ref().unwrap());
        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        tokio::spawn(async move {
            loop {
                let mut locked_subscribe_blocks = subscribe_blocks.lock().await;
                for (_, subscribe_block) in locked_subscribe_blocks.iter_mut() {
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
