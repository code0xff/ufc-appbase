use std::sync::Arc;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use rocksdb::{DB, DBWithThreadMode, SingleThreaded};

pub struct RocksPlugin {
    base: PluginBase,
    db: Option<RocksDB>,
    monitor: Option<SubscribeHandle>,
}

type RocksDB = Arc<FutureMutex<DBWithThreadMode<SingleThreaded>>>;

appbase_plugin_requires!(RocksPlugin; );

impl Plugin for RocksPlugin {
    appbase_plugin_default!(RocksPlugin);

    fn new() -> Self {
        RocksPlugin {
            base: PluginBase::new(),
            db: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }

        self.db = Some(Arc::new(FutureMutex::new(DB::open_default("rocks").unwrap())));
        self.monitor = Some(app::subscribe_channel(String::from("rocks")));
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
                    let key = String::from(data.get("key").unwrap().as_str().unwrap());
                    let value = String::from(data.get("value").unwrap().as_str().unwrap());

                    let _ = locked_db.put(key.clone(), value);
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
