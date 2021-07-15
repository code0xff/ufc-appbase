use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use appbase::*;
use jsonrpc_core::{Params, Value};
use serde_json;
use serde_json::Map;

use crate::plugin::jsonrpc::JsonRpcPlugin;

pub struct MonitorPlugin {
    base: PluginBase,
    tendermint: Option<ChannelHandle>,
    // channels: Option<Arc<Mutex<HashMap<String, ChannelHandle>>>>,
}

appbase_plugin_requires!(MonitorPlugin; JsonRpcPlugin);

impl Plugin for MonitorPlugin {
    appbase_plugin_default!(MonitorPlugin);

    fn new() -> Self {
        MonitorPlugin {
            base: PluginBase::new(),
            tendermint: None,
            // channels: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        unsafe {
            self.tendermint = Some(APP.get_channel("tendermint".to_string()));
            // let mut channels: HashMap<String, ChannelHandle> = HashMap::new();
            // channels.insert("tendermint".to_string(), APP.get_channel("tendermint".to_string()));
            // self.channels = Some(Arc::new(Mutex::new(channels.to_owned())));
        }
        let tendermint = Arc::clone(&self.tendermint.as_ref().unwrap());

        let mut _p1: PluginHandle;
        unsafe {
            _p1 = APP.get_plugin::<JsonRpcPlugin>();
        }
        let mut plugin = _p1.lock().unwrap();
        let jsonrpc = plugin.downcast_mut::<JsonRpcPlugin>().unwrap();
        jsonrpc.add_sync_method("subscribe_block".to_string(), move |params: Params| {
            let _params: Map<String, Value> = params.parse().unwrap();
            let chain = _params.get("chain").unwrap().as_str().unwrap();

            tendermint
                .lock()
                .unwrap()
                .send(Value::String("start!".to_string()))
                .unwrap();

            Ok(Value::String("subscribe started".to_string()))
        });
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
    }
}
