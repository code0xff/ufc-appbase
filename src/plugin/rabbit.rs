use std::sync::Arc;

use amiquip::{Connection, Exchange, Publish};
use appbase::*;
use futures::lock::Mutex as FutureMutex;

pub struct RabbitPlugin {
    base: PluginBase,
    conn: Option<RabbitConnection>,
    monitor: Option<SubscribeHandle>,
}

type RabbitConnection = Arc<FutureMutex<Connection>>;

appbase_plugin_requires!(RabbitPlugin; );

impl Plugin for RabbitPlugin {
    appbase_plugin_default!(RabbitPlugin);

    fn new() -> Self {
        RabbitPlugin {
            base: PluginBase::new(),
            conn: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        self.conn = Some(Arc::new(FutureMutex::new(Connection::insecure_open("amqp://rabbitmq:rabbitmq@localhost:5672").unwrap())));
        self.monitor = Some(app::subscribe_channel(String::from("rabbit")));
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let conn = Arc::clone(self.conn.as_ref().unwrap());
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            let channel = conn.lock().await.open_channel(None).unwrap();
            let exchange = Exchange::direct(&channel);
            loop {
                if let Ok(msg) = mon_lock.try_recv() {
                    let _ = exchange.publish(Publish::new(msg.as_str().unwrap().as_bytes(), "ufc"));
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
