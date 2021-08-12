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
    db: Option<Database>,
    monitor: Option<channel::Receiver>,
}

plugin::requires!(MongoPlugin; );

message!(MongoMsg; {collection: String}, {document: Value});

impl Plugin for MongoPlugin {
    fn new() -> Self {
        app::arg(clap::Arg::new("mongo::url").long("mongo-url").takes_value(true));

        MongoPlugin {
            db: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        let mongo_url = libs::opts::string("mongo::url").unwrap();
        let mut client_opts = executor::block_on(async { ClientOptions::parse(mongo_url).await }).unwrap();
        client_opts.app_name = Some(String::from("MongoDB"));
        let client = Client::with_options(client_opts).unwrap();
        self.db = Some(client.database("ufc"));
        self.monitor = Some(app::subscribe_channel(String::from("mongo")));
    }

    fn startup(&mut self) {
        let db = self.db.as_ref().unwrap().clone();
        let monitor = self.monitor.take().unwrap();
        let app = app::quit_handle().unwrap();
        Self::recv(db, monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl MongoPlugin {
    fn recv(db: Database, mut monitor: channel::Receiver, app: QuitHandle) {
        app::spawn(async move {
            if let Ok(msg) = monitor.try_recv() {
                let parsed_msg = msg.as_object().unwrap();
                let collection_name = get_str(parsed_msg, "collection").unwrap();
                let value = get_object(parsed_msg, "document").unwrap();

                let collection = db.collection::<Document>(collection_name);
                let document = libs::mongo::get_doc(value);
                let result = collection.insert_one(document.clone(), None).await;
                if let Err(err) = result {
                    println!("mongo_error={:?}", err);
                }
            }
            if !app.is_quiting() {
                Self::recv(db, monitor, app);
            }
        });
    }
}
