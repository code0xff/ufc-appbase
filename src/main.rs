use appbase::app;

use crate::plugin::tendermint::TendermintPlugin;

mod plugin;
mod types;
mod validation;
mod libs;
mod error;

fn main() {
    env_logger::init();
    dotenv::dotenv().ok();
    app::register_plugin::<TendermintPlugin>();
    app::initialize!(TendermintPlugin);
    app::startup();
    app::execute();
}
