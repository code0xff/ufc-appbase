use std::collections::HashMap;

use appbase::*;
use mysql::*;
use mysql::prelude::Queryable;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::libs::environment;
use crate::libs::mysql::get_params;
use crate::libs::serde::{get_object, get_str};
use crate::message;
use crate::plugin::jsonrpc::JsonRpcPlugin;

pub struct MySqlPlugin {
    pool: Option<Pool>,
    monitor: Option<SubscribeHandle>,
}

message!(MySqlMsg; {query: String}, {value: Value});

appbase_plugin_requires!(MySqlPlugin; JsonRpcPlugin);

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
        Self::recv(pool, monitor, app);
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
                    if let Err(err) = result {
                        println!("mysql_error={:?}", err);
                    }
                }
            }
            if !app.is_quiting() {
                Self::recv(pool, monitor, app);
            }
        });
    }

    pub fn execute(&self, query: String, params: Params) -> Result<()> {
        let pool = self.pool.as_ref().unwrap();
        let _ = pool.get_conn().unwrap().exec_drop(query, params)?;
        Ok(())
    }

    pub fn query(&self, query: String, params: Params) -> Result<Vec<Value>> {
        let pool = self.pool.as_ref().unwrap();
        let rows: Vec<Row> = pool.get_conn().unwrap().exec(query, params)?;

        let mut result: Vec<Value> = Vec::new();
        for row in rows.iter() {
            let mut converted = HashMap::new();
            for column in row.columns_ref() {
                let column_value = &row[column.name_str().as_ref()];
                converted.insert(String::from(column.name_str()), column_value.as_sql(true));
            }
            result.push(json!(converted));
        }
        Ok(result)
    }
}
