use appbase::*;

use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::monitor::MonitorPlugin;
use std::sync::Arc;

pub struct TendermintPlugin {
    base: PluginBase,
    monitor: Option<SubscribeHandle>,
}

appbase_plugin_requires!(TendermintPlugin; JsonRpcPlugin, MonitorPlugin);

impl Plugin for TendermintPlugin {
    appbase_plugin_default!(TendermintPlugin);

    fn new() -> Self {
        TendermintPlugin {
            base: PluginBase::new(),
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        unsafe {
            self.monitor = Some(APP.subscribe_channel("tendermint".to_string()));
        }
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }

        let _m1 = Arc::clone(self.monitor.as_ref().unwrap());
        tokio::spawn(async move {
            let mut monitor = _m1.lock().await;
            loop {
                let message = monitor.recv().await.unwrap();
                println!("{:?}", message);
            }
        });
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
    }
}
