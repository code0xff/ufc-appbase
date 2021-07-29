use std::sync::Arc;

use appbase::*;
use futures::executor;
use futures::lock::Mutex as FutureMutex;
use mongodb::{Client, Database};
use mongodb::bson;
use mongodb::bson::*;
use mongodb::options::ClientOptions;

use crate::{unwrap, get_string};

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

        let mut client_opts = executor::block_on(async { ClientOptions::parse("mongodb://localhost:27017").await }).unwrap();
        client_opts.app_name = Some(String::from("MongoDB"));
        let client = Client::with_options(client_opts).unwrap();
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
            let mut mon_lock = monitor.lock().await;
            loop {
                let db_lock = db.lock().await;
                if let Ok(msg) = mon_lock.try_recv() {
                    let map = msg.as_object().unwrap();
                    let coll_nm = get_string!(map; "collection").unwrap();

                    let coll = db_lock.collection::<Document>(coll_nm.as_str());
                    let doc = bson::to_document(&map).unwrap();
                    println!("{:?}", doc);

                    let _ = coll.insert_one(doc, None).await;
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
