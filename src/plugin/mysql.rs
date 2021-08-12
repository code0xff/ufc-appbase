use appbase::*;
use mysql::*;
use mysql::prelude::Queryable;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value, Number};

use crate::libs::mysql::get_params;
use crate::libs::opts;
use crate::libs::serde::{get_object, get_str};
use crate::message;
use crate::plugin::jsonrpc::JsonRpcPlugin;

pub struct MySqlPlugin {
    pool: Option<Pool>,
    monitor: Option<channel::Receiver>,
}

message!(MySqlMsg; {query: String}, {value: Value});

plugin::requires!(MySqlPlugin; JsonRpcPlugin);

impl Plugin for MySqlPlugin {
    fn new() -> Self {
        app::arg(clap::Arg::new("mysql::url").long("mysql-url").takes_value(true));

        MySqlPlugin {
            monitor: None,
            pool: None,
        }
    }

    fn initialize(&mut self) {
        let mysql_url = opts::string("mysql::url").unwrap();
        let opts = Opts::from_url(mysql_url.as_str()).unwrap();
        let pool = Pool::new(opts).unwrap();
        self.pool = Some(pool);
        self.monitor = Some(app::subscribe_channel(String::from("mysql")));
    }

    fn startup(&mut self) {
        let pool = self.pool.as_ref().unwrap().clone();
        let monitor = self.monitor.take().unwrap();
        let app = app::quit_handle().unwrap();
        Self::recv(pool, monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl MySqlPlugin {
    fn recv(pool: Pool, mut monitor: channel::Receiver, app: QuitHandle) {
        app::spawn(async move {
            if let Ok(msg) = monitor.try_recv() {
                let parsed_msg = msg.as_object().unwrap();
                let query = get_str(parsed_msg, "query").unwrap();
                let value = get_object(parsed_msg, "value").unwrap();
                let params = get_params(value);
                let result = pool.get_conn().unwrap().exec_drop(query, params);
                if let Err(err) = result {
                    println!("mysql_error={:?}", err);
                }
            }
            if !app.is_quiting() {
                Self::recv(pool, monitor, app);
            }
        });
    }

    pub fn get_pool(&self) -> Pool {
        self.pool.as_ref().unwrap().clone()
    }

    pub fn execute(&self, query: String, params: Params) -> Result<()> {
        let pool = self.pool.as_ref().unwrap();
        let _ = pool.get_conn().unwrap().exec_drop(query, params)?;
        Ok(())
    }

    pub fn query_static(pool: &Pool, query: String, params: Params) -> Result<Vec<Value>> {
        let rows: Vec<Row> = pool.get_conn().unwrap().exec(query, params)?;

        let mut converted_rows: Vec<Value> = Vec::new();
        for raw_row in rows.iter() {
            let mut converted_row = Map::new();
            for column in raw_row.columns_ref() {
                let column_value = &raw_row[column.name_str().as_ref()];
                let value = match column_value {
                    mysql::Value::NULL => serde_json::Value::Null,
                    mysql::Value::Bytes(b) => serde_json::Value::String(String::from_utf8(b.clone()).unwrap()),
                    mysql::Value::Int(i) => serde_json::Value::Number(Number::from(*i)),
                    mysql::Value::UInt(ui) => serde_json::Value::Number(Number::from(*ui)),
                    mysql::Value::Float(f) => serde_json::Value::Number(Number::from_f64(*f as f64).unwrap()),
                    mysql::Value::Double(d) => serde_json::Value::Number(Number::from_f64(*d).unwrap()),
                    mysql::Value::Date(y, m, d, h, mi, s, ms) =>
                        serde_json::Value::String(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:06}", y, m, d, h, mi, s, ms)),
                    mysql::Value::Time(n, d, h, mi, s, ms) => {
                        if *n {
                            serde_json::Value::String(format!("-{:03}:{:02}:{:02}.{:06}", d * 24 + u32::from(*h), mi, s, ms))
                        } else {
                            serde_json::Value::String(format!("{:03}:{:02}:{:02}.{:06}", d * 24 + u32::from(*h), mi, s, ms))
                        }
                    }
                };
                converted_row.insert(String::from(column.name_str()), value);
            }
            converted_rows.push(Value::Object(converted_row));
        }
        Ok(converted_rows)
    }
}
