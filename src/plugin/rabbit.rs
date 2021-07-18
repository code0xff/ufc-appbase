use std::sync::Arc;

use amiquip::{Connection, Exchange, Publish};
use appbase::*;
use futures::executor;
use futures::lock::Mutex as FutureMutex;
use mongodb::{Client, Database};
use mongodb::bson;
use mongodb::bson::*;
use mongodb::options::ClientOptions;

pub struct RabbitPlugin {
    base: PluginBase,
    connection: Option<RabbitConnection>,
    monitor: Option<SubscribeHandle>,
}

type RabbitConnection = Arc<FutureMutex<Connection>>;

appbase_plugin_requires!(RabbitPlugin; );

impl Plugin for RabbitPlugin {
    appbase_plugin_default!(RabbitPlugin);

    fn new() -> Self {
        RabbitPlugin {
            base: PluginBase::new(),
            connection: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        unsafe {
            self.connection = Some(Arc::new(FutureMutex::new(Connection::insecure_open("amqp://rabbitmq:rabbitmq@localhost:5672").unwrap())));
            self.monitor = Some(APP.subscribe_channel(String::from("rabbit")));
        }
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let connection = Arc::clone(self.connection.as_ref().unwrap());
        tokio::spawn(async move {
            let mut _monitor = monitor.lock().await;
            let mut channel = connection.lock().await.open_channel(None).unwrap();
            let exchange = Exchange::direct(&channel);
            loop {
                if let Ok(message) = _monitor.try_recv() {
                    println!("{:?}", message.to_string());
                    // exchange.publish(Publish::new(message.to_string().as_str().as_bytes(), "ufc"));
                }
            }
        });
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
        let connection = Arc::clone(self.connection.as_ref().unwrap());
        futures::executor::block_on(async move { connection.lock().await.close() });
    }
}
