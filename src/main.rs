use appbase::app;

use crate::plugin::tendermint::TendermintPlugin;
use crate::plugin::ethereum::EthereumPlugin;

mod plugin;
mod types;
mod validation;
mod libs;
mod error;

fn main() {
    env_logger::init();
    dotenv::dotenv().ok();
    app::register_plugin::<EthereumPlugin>();
    app::register_plugin::<TendermintPlugin>();
    app::register_plugin::<plugin::email::EmailPlugin>();
    app::register_plugin::<plugin::mongo::MongoPlugin>();
    app::register_plugin::<plugin::mysql::MySqlPlugin>();
    app::register_plugin::<plugin::rabbit::RabbitPlugin>();
    app::register_plugin::<plugin::slack::SlackPlugin>();
    app::register_plugin::<plugin::telegram::TelegramPlugin>();
    app::initialize!(TendermintPlugin);
    app::initialize!(EthereumPlugin);
    app::startup();
    app::execute();
}
