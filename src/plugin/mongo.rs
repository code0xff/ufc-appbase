use appbase::*;
use futures::executor;
use mongodb::{Client, Database};
use mongodb::bson::*;
use mongodb::options::ClientOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{libs, message};
use crate::libs::serde::{get_object, get_str};

pub struct MongoPlugin {
    db: Option<MongoDB>,
    monitor: Option<SubscribeHandle>,
}

type MongoDB = Database;

appbase_plugin_requires!(MongoPlugin; );

message!(MongoMsg; {collection: String}, {document: Value});

impl Plugin for MongoPlugin {
    fn new() -> Self {
        MongoPlugin {
            db: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        let mut client_opts = executor::block_on(async { ClientOptions::parse("mongodb://root:mongodb@localhost:27017").await }).unwrap();
        client_opts.app_name = Some(String::from("MongoDB"));
        let client = Client::with_options(client_opts).unwrap();
        self.db = Some(client.database("ufc"));
        self.monitor = Some(app::subscribe_channel(String::from("mongo")));
    }

    fn startup(&mut self) {
        let db = self.db.as_ref().unwrap().clone();
        let monitor = self.monitor.as_ref().unwrap().clone();
        let app = app::quit_handle().unwrap();
        MongoPlugin::recv(db, monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl MongoPlugin {
    fn recv(db: MongoDB, monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let collection_name = get_str(parsed_msg, "collection").unwrap();
                    let value = get_object(parsed_msg, "document").unwrap();

                    let collection = db.collection::<Document>(collection_name);
                    let document = libs::mongo::get_doc(value);
                    let _ = collection.insert_one(document.clone(), None).await;
                }
            }
            if !app.is_quiting() {
                MongoPlugin::recv(db, monitor, app);
            }
        });
    }
}