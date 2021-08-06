use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use jsonrpc_core::{IoHandler, RpcMethodSimple, RpcMethodSync};
use jsonrpc_http_server::{CloseHandle, ServerBuilder};

use appbase::*;

pub struct JsonRpcPlugin {
    io: Option<IoHandler>,
    server: Option<CloseHandle>,
}

appbase_plugin_requires!(JsonRpcPlugin; );

/*
 * `add_sync_method` and `add_method` SHOULD be called during plugin initialization.
 * After JsonRpcPlugin starts, IoHandler moves into closure, so not available to access from plugin.
 */
impl JsonRpcPlugin {
    #[allow(dead_code)]
    pub fn add_sync_method<F>(&mut self, name: String, func: F) where F: RpcMethodSync {
        match self.io.as_mut() {
            Some(io) => io.add_sync_method(name.as_str(), func),
            None => log::error!("add method not available"),
        }
    }

    #[allow(dead_code)]
    pub fn add_method<F>(&mut self, name: String, func: F) where F: RpcMethodSimple {
        match self.io.as_mut() {
            Some(io) => io.add_method(name.as_str(), func),
            None => log::error!("add method not available"),
        }
    }
}

impl Plugin for JsonRpcPlugin {
    fn new() -> Self {
        JsonRpcPlugin {
            io: None,
            server: None,
        }
    }

    fn initialize(&mut self) {
        self.io = Some(IoHandler::new());
    }

    fn startup(&mut self) {
        let io = self.io.take().unwrap();
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        if let Ok(server) = ServerBuilder::new(io).start_http(&socket) {
            self.server = Some(server.close_handle());
            tokio::task::spawn_blocking(|| {
                server.wait();
            });
        }
    }

    fn shutdown(&mut self) {
        if let Some(server) = self.server.take() {
            server.close();
        }
    }
}
