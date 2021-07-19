use std::sync::Arc;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use mongodb::{Client, Database};
use mongodb::bson;
use mongodb::bson::*;
use mongodb::options::ClientOptions;
use futures::executor;

pub struct MongoPlugin {
    base: PluginBase,
    db: Option<MongoDB>,
    monitor: Option<SubscribeHandle>,
}

type MongoDB = Arc<FutureMutex<Database>>;

appbase_plugin_requires!(MongoPlugin; );

impl Plugin for MongoPlugin {
    appbase_plugin_default!(MongoPlugin);

    fn new() -> Self {
        MongoPlugin {
            base: PluginBase::new(),
            db: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        let mut client_options = executor::block_on(async { ClientOptions::parse("mongodb://localhost:27017").await }).unwrap();
        client_options.app_name = Some(String::from("MongoDB"));
        let client = Client::with_options(client_options).unwrap();
        self.db = Some(Arc::new(FutureMutex::new(client.database("ufc"))));
        self.monitor = Some(app::subscribe_channel(String::from("mongo")));
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let db = Arc::clone(self.db.as_ref().unwrap());
        tokio::spawn(async move {
            let mut locked_monitor = monitor.lock().await;
            loop {
                let locked_db = db.lock().await;
                if let Ok(message) = locked_monitor.try_recv() {
                    let data = message.as_object().unwrap();
                    let collection_name = String::from(data.get("collection").unwrap().as_str().unwrap());

                    let collection = locked_db.collection::<Document>(collection_name.as_str());
                    let document = bson::to_document(&data).unwrap();

                    println!("{:?}", document);
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
