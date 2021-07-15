use appbase::APP;

use crate::plugin::tendermint::TendermintPlugin;

mod plugin;

#[tokio::main]
async fn main() {
    env_logger::init();
    unsafe {
        APP.register_plugin::<TendermintPlugin>();
        APP.initialize();
        APP.startup();
        APP.execute().await; // XXX: a better way for graceful shutdown?
    }
}
