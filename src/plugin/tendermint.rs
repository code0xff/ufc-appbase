use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use appbase::*;
use serde_json::{Map, Value};

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::monitor::MonitorPlugin;
use crate::types::block::SubscribeBlock;
use jsonrpc_http_server::tokio::time::Duration;
use std::thread;

pub struct TendermintPlugin {
    base: PluginBase,
    subscribe_blocks: Option<SubscribeBlocks>,
    monitor: Option<SubscribeHandle>,
}

type SubscribeBlocks = Arc<Mutex<Vec<SubscribeBlock>>>;

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, MonitorPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            subscribe_blocks: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        unsafe {
            self.subscribe_blocks = Some(Arc::new(Mutex::new(Vec::new())));
            self.monitor = Some(APP.subscribe_channel("tendermint".to_string()));
        }
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }

        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        tokio::spawn(async move {
            let mut _monitor = monitor.lock().await;
            loop {
                let message = _monitor.recv().await.unwrap();
                let block = message.as_object().unwrap();
                let chain = block.get("chain").unwrap().to_string();
                let chain_id = block.get("chain_id").unwrap().to_string();
                let start_height = block.get("start_height").unwrap().as_u64().unwrap();
                let _nodes = block.get("nodes").unwrap().as_array().unwrap();
                let mut nodes: Vec<String> = Vec::new();
                for n in _nodes.iter() {
                    nodes.push(String::from(n.as_str().unwrap()));
                }
                let subscribe_block = SubscribeBlock {
                    chain,
                    chain_id,
                    start_height,
                    current_height: start_height,
                    nodes,
                    node_index: 0,
                };
                subscribe_blocks.lock().unwrap().push(subscribe_block);
            }
        });

        let subscribe_blocks = Arc::clone(self.subscribe_blocks.as_ref().unwrap());
        tokio::spawn(async move {
            loop {
                let mut _subscribe_blocks = subscribe_blocks.lock().unwrap().clone();
                for sb in _subscribe_blocks.iter_mut() {
                    if sb.node_index >= 0 {
                        let node_index = usize::try_from(sb.node_index).unwrap();
                        let node_url = sb.nodes[node_index].as_str().to_owned() + sb.current_height.to_string().as_str();

                        let body = reqwest::get(node_url)
                            .await
                            .unwrap()
                            .text()
                            .await;

                        let result: Map<String, Value> = serde_json::from_str(body.unwrap().as_str()).unwrap();
                        if result.get("error").is_none() {
                            sb.current_height += 1;
                            println!("{:?}", result);
                        } else {
                            println!("{:?}", result.get("error").unwrap());
                        }
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
    }
}
