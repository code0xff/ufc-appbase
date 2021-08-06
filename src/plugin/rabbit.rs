use std::sync::Arc;

use amiquip::{Connection, Exchange, Publish};
use appbase::*;
use futures::lock::Mutex as FutureMutex;

use crate::libs::environment;

pub struct RabbitPlugin {
    conn: Option<RabbitConnection>,
    monitor: Option<SubscribeHandle>,
}

type RabbitConnection = Arc<FutureMutex<Connection>>;

appbase_plugin_requires!(RabbitPlugin; );

impl Plugin for RabbitPlugin {
    fn new() -> Self {
        RabbitPlugin {
            conn: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        let rabbit_mq_url = environment::string("RABBIT_MQ_URL").unwrap();
        self.conn = Some(Arc::new(FutureMutex::new(Connection::insecure_open(rabbit_mq_url.as_str()).unwrap())));
        self.monitor = Some(app::subscribe_channel(String::from("rabbit")));
    }

    fn startup(&mut self) {
        let conn = self.conn.as_ref().unwrap().clone();
        let monitor = self.monitor.as_ref().unwrap().clone();
        let app = app::quit_handle().unwrap();
        RabbitPlugin::recv(conn, monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl RabbitPlugin {
    fn recv(conn: RabbitConnection, monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Some(mut conn_lock) = conn.try_lock() {
                    let channel = conn_lock.open_channel(None).unwrap();
                    let exchange = Exchange::direct(&channel);
                    if let Ok(msg) = mon_lock.try_recv() {
                        let queue = environment::string("RABBIT_MQ_QUEUE").unwrap();
                        let _ = exchange.publish(Publish::new(msg.as_str().unwrap().as_bytes(), queue.as_str()));
                    }
                }
            }
            if !app.is_quiting() {
                RabbitPlugin::recv(conn, monitor, app);
            }
        });
    }
}
