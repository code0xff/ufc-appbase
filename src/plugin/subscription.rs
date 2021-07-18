use std::collections::HashMap;
use std::sync::Arc;

use appbase::*;
use jsonrpc_core::{Params, Value};
use serde_json;
use serde_json::Map;

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::validation::get_subs;
use crate::validation::subscribe;
use crate::validation::unsubscribe;
use crate::plugin::tendermint::TendermintPlugin;

pub struct SubscriptionPlugin {
    base: PluginBase,
    channels: Option<HashMap<String, ChannelHandle>>,
}


appbase_plugin_requires!(SubscriptionPlugin; JsonRpcPlugin, TendermintPlugin);

impl Plugin for SubscriptionPlugin {
    appbase_plugin_default!(SubscriptionPlugin);

    fn new() -> Self {
        SubscriptionPlugin {
            base: PluginBase::new(),
            channels: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        unsafe {
            let mut channels: HashMap<String, ChannelHandle> = HashMap::new();
            channels.insert(String::from("tendermint"), APP.get_channel(String::from("tendermint")));
            self.channels = Some(channels.to_owned());
        }
        let channels = self.channels.as_ref().unwrap().clone();

        let plugin_handle: PluginHandle;
        unsafe {
            plugin_handle = APP.get_plugin::<JsonRpcPlugin>();
        }
        let mut plugin = plugin_handle.lock().unwrap();
        let jsonrpc = plugin.downcast_mut::<JsonRpcPlugin>().unwrap();
        jsonrpc.add_sync_method(String::from("subscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                return Ok(Value::String(verified.unwrap_err()));
            }

            let chain = params.get("chain").unwrap().as_str().unwrap();
            let result = match channels.get(chain) {
                None => {
                    Err(String::from("not registered chain"))
                }
                Some(channel) => {
                    let mut channel_message: Map<String, Value> = Map::new();
                    channel_message.insert(String::from("method"), Value::String(String::from("subscribe")));
                    channel_message.insert(String::from("params"), Value::Object(params));

                    channel
                        .lock()
                        .unwrap()
                        .send(Value::Object(channel_message))
                        .unwrap();
                    Ok(String::from("subscribe started"))
                }
            };

            let result_message = match result {
                Ok(message) => message,
                Err(err_message) => err_message,
            };
            Ok(Value::String(result_message))
        });

        jsonrpc.add_sync_method(String::from("unsubscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Ok(Value::String(verified.unwrap_err()));
            }

            let chain = params.get("chain").unwrap().as_str().unwrap();
            let result = match channels.get(chain) {
                None => {
                    Err(String::from("not registered chain"))
                }
                Some(channel) => {
                    let mut channel_message: Map<String, Value> = Map::new();
                    channel_message.insert(String::from("method"), Value::String(String::from("unsubscribe")));
                    channel_message.insert(String::from("params"), Value::Object(params));

                    channel
                        .lock()
                        .unwrap()
                        .send(Value::Object(channel_message))
                        .unwrap();
                    Ok(String::from("unsubscribed"))
                }
            };

            let result_message = match result {
                Ok(message) => message,
                Err(err_message) => err_message,
            };
            Ok(Value::String(result_message))
        });

        // jsonrpc.add_sync_method(String::from("get_subs"), move |params: Params| {
        //     let params: Map<String, Value> = params.parse().unwrap();
        //     let verified = get_subs::verify(&params);
        //     if verified.is_err() {
        //         return Ok(Value::String(verified.unwrap_err()));
        //     }
        //
        //     let chain = params.get("chain").unwrap().as_str().unwrap();
        //
        //     let result_message = match result {
        //         Ok(message) => message,
        //         Err(err_message) => err_message,
        //     };
        //     Ok(Value::String(result_message))
        // });
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
