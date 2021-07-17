use std::convert::TryFrom;
use std::sync::Arc;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::monitor::MonitorPlugin;
use crate::plugin::rocks::RocksPlugin;
use crate::plugin::mongo::MongoPlugin;
use crate::types::block::SubscribeBlock;

pub struct TendermintPlugin {
    base: PluginBase,
    subscribe_blocks: Option<SubscribeBlocks>,
    channel: Option<ChannelHandle>,
    monitor: Option<SubscribeHandle>,
}

type SubscribeBlocks = Arc<FutureMutex<Vec<SubscribeBlock>>>;

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, MonitorPlugin, MongoPlugin);

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
            self.channel = Some(APP.get_channel(String::from("mongo")));
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
            let mut _monitor = monitor.lock().await;
            loop {
                if let Ok(message) = _monitor.try_recv() {
                    let block = message.as_object().unwrap();
                    let _nodes = block.get("nodes").unwrap().as_array().unwrap();
                    let start_height = block.get("start_height").unwrap().as_u64().unwrap();
                    let mut nodes: Vec<String> = Vec::new();
                    for n in _nodes.iter() {
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
                    let mut _subscribe_blocks = subscribe_blocks.lock().await;
                    _subscribe_blocks.push(new_subscribe_block);
                }

                let mut _subscribe_blocks = subscribe_blocks.lock().await;
                for subscribe_block in _subscribe_blocks.iter_mut() {
                    if subscribe_block.node_index >= 0 {
                        let node_index = usize::try_from(subscribe_block.node_index).unwrap();
                        let node_url = subscribe_block.nodes[node_index].as_str().to_owned() + subscribe_block.current_height.to_string().as_str();

                        let body = reqwest::get(node_url)
                            .await
                            .unwrap()
                            .text()
                            .await;

                        let _body: Map<String, Value> = serde_json::from_str(body.unwrap().as_str()).unwrap();
                        if _body.get("error").is_none() {
                            let result = _body.get("result").unwrap().as_object().unwrap();
                            let block = result.get("block").unwrap().as_object().unwrap();
                            let block_header = block.get("header").unwrap();

                            subscribe_block.current_height += 1;

                            let mut data = Map::new();
                            data.insert(String::from("collection"), Value::String(String::from("block")));
                            data.insert(String::from("document"), block_header.clone());

                            channel.lock().unwrap().send(Value::Object(data));

                            // let key = format!("{}:{}:{}", subscribe_block.chain, subscribe_block.chain_id, subscribe_block.current_height);
                            // data.insert(String::from("key"), Value::String(key));
                            // data.insert(String::from("value"), Value::String(block_header.to_string()));
                            //
                            // let _ = channel.lock().unwrap().send(Value::Object(data));
                        } else {
                            println!("{:?}", _body.get("error").unwrap());
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
