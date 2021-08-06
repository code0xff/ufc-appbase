use std::fs;

use appbase::*;
use mysql::*;
use mysql::prelude::Queryable;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{message};
use crate::libs::environment;
use crate::libs::mysql::get_params;
use crate::libs::serde::{get_object, get_str};
use crate::plugin::jsonrpc::JsonRpcPlugin;

pub struct MySqlPlugin {
    pool: Option<MySqlPool>,
    monitor: Option<SubscribeHandle>,
}

type MySqlPool = Pool;

message!(MySqlMsg; {query: String}, {value: Value});

appbase_plugin_requires!(MySqlPlugin; JsonRpcPlugin);

impl MySqlPlugin {
    pub fn create_table(&self, sql_files: Vec<&str>) {
        let pool = self.pool.as_ref().unwrap();
        for sql_file in sql_files.into_iter() {
            let query = fs::read_to_string(sql_file).unwrap();
            let result = pool.get_conn().unwrap().exec_drop(query, ());
            match result {
                Ok(_) => {}
                Err(err) => {
                    println!("error={:?}", err);
                }
            }
        }
    }
}

impl Plugin for MySqlPlugin {
    fn new() -> Self {
        MySqlPlugin {
            monitor: None,
            pool: None,
        }
    }

    fn initialize(&mut self) {
        let mysql_url = environment::string("MYSQL_URL").unwrap();
        let opts = Opts::from_url(mysql_url.as_str()).unwrap();
        let pool = Pool::new(opts).unwrap();
        self.pool = Some(pool);
        self.monitor = Some(app::subscribe_channel(String::from("mysql")));
    }

    fn startup(&mut self) {
        let pool = self.pool.as_ref().unwrap().clone();
        let monitor = self.monitor.as_ref().unwrap().clone();
        let app = app::quit_handle().unwrap();
        MySqlPlugin::recv(pool, monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl MySqlPlugin {
    fn recv(pool: Pool, monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let query = get_str(parsed_msg, "query").unwrap();
                    let value = get_object(parsed_msg, "value").unwrap();
                    let params = get_params(value);
                    let result = pool.get_conn().unwrap().exec_drop(query, params);
                    if result.is_err() {
                        println!("{}", result.unwrap_err());
                    }
                }
            }
            if !app.is_quiting() {
                MySqlPlugin::recv(pool, monitor, app);
            }
        });
    }
}