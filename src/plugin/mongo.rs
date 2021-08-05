use std::sync::Arc;

use appbase::*;
use futures::executor;
use futures::lock::Mutex as FutureMutex;
use mongodb::{Client, Database};
use mongodb::bson::*;
use mongodb::options::ClientOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{libs, message};
use crate::libs::serde::{get_object, get_str};

pub struct MongoPlugin {
    base: PluginBase,
    db: Option<MongoDB>,
    monitor: Option<SubscribeHandle>,
}

type MongoDB = Arc<FutureMutex<Database>>;

appbase_plugin_requires!(MongoPlugin; );

message!(MongoMsg; {collection: String}, {document: Value});

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
        let mut client_opts = executor::block_on(async { ClientOptions::parse("mongodb://root:mongodb@localhost:27017").await }).unwrap();
        client_opts.app_name = Some(String::from("MongoDB"));
        let client = Client::with_options(client_opts).unwrap();
        self.db = Some(Arc::new(FutureMutex::new(client.database("ufc"))));
        self.monitor = Some(app::subscribe_channel(String::from("mongo")));
    }

    fn startup(&mut self) {
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let db = Arc::clone(self.db.as_ref().unwrap());
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                let db_lock = db.lock().await;
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let collection_name = get_str(parsed_msg, "collection").unwrap();
                    let value = get_object(parsed_msg, "document").unwrap();

                    let collection = db_lock.collection::<Document>(collection_name);
                    let document = libs::mongo::get_doc(value);
                    let _ = collection.insert_one(document.clone(), None).await;
                }
            }
        });
    }

    fn shutdown(&mut self) {}
}
