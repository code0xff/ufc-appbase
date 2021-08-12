use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use std::sync::Arc;

use appbase::*;
use appbase::plugin::State;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::Params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{enumeration, libs, message};
use crate::error::error::ExpectedError;
use crate::libs::mysql::get_params;
use crate::libs::request;
use crate::libs::rocks::{get_by_prefix_static, get_static};
use crate::libs::serde::{get_array, get_object, get_str, get_value_by_path, get_string, select_value};
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mongo::{MongoMsg, MongoPlugin};
use crate::plugin::mysql::{MySqlMsg, MySqlPlugin};
use crate::plugin::rocks::{RocksMethod, RocksMsg, RocksPlugin};
use crate::types::channel::MultiChannel;
use crate::types::enumeration::Enumeration;
use crate::types::mysql::Schema;
use crate::types::subscribe::{SubscribeEvent, SubscribeStatus, SubscribeTarget, SubscribeTask};
use crate::validation::{get_blocks, get_task, get_txs, resubscribe, stop_subscribe, subscribe, unsubscribe};

pub struct TendermintPlugin {
    sub_events: Option<SubscribeEvents>,
    channels: Option<MultiChannel>,
    monitor: Option<channel::Receiver>,
    schema: Option<HashMap<String, Schema>>,
    tm_block_mysql_sync: Option<bool>,
    tm_tx_mysql_sync: Option<bool>,
    tm_block_mongo_sync: Option<bool>,
    tm_tx_mongo_sync: Option<bool>,
    tm_block_publish: Option<bool>,
    tm_tx_publish: Option<bool>,
}

const CHAIN: &str = "tendermint";
const TASK_PREFIX: &str = "task:tendermint";

type SubscribeEvents = Arc<FutureMutex<HashMap<String, SubscribeEvent>>>;

message!((TendermintMsg; {value: Value}); (TendermintMethod; {Subscribe: "subscribe"}, {Resubscribe: "resubscribe"}, {Stop: "stop"}, {Unsubscribe: "unsubscribe"}));

plugin::requires!(TendermintPlugin; JsonRpcPlugin, RocksPlugin);

impl Plugin for TendermintPlugin {
    fn new() -> Self {
        app::arg(clap::Arg::new("tendermint::block-mysql-sync").long("tm-block-mysql-sync"));
        app::arg(clap::Arg::new("tendermint::tx-mysql-sync").long("tm-tx-mysql-sync"));
        app::arg(clap::Arg::new("tendermint::block-mongo-sync").long("tm-block-mongo-sync"));
        app::arg(clap::Arg::new("tendermint::tx-mongo-sync").long("tm-tx-mongo-sync"));
        app::arg(clap::Arg::new("tendermint::block-rabbit-mq-publish").long("tm-block-rabbit-mq-publish"));
        app::arg(clap::Arg::new("tendermint::tx-rabbit-mq-publish").long("tm-tx-rabbit-mq-publish"));

        TendermintPlugin {
            sub_events: None,
            channels: None,
            monitor: None,
            schema: None,
            tm_block_mysql_sync: None,
            tm_tx_mysql_sync: None,
            tm_block_mongo_sync: None,
            tm_tx_mongo_sync: None,
            tm_block_publish: None,
            tm_tx_publish: None,
        }
    }

    fn initialize(&mut self) {
        self.init();
        self.register_jsonrpc();
        self.load_tasks();
        self.init_mysql();
    }

    fn startup(&mut self) {
        let mut monitor = self.monitor.take().unwrap();
        let sub_events = Arc::clone(self.sub_events.as_ref().unwrap());

        let rocks_channel = self.channels.as_ref().unwrap().get("rocks");
        let mysql_channel = self.channels.as_ref().unwrap().get("mysql");
        let rabbit_channel = self.channels.as_ref().unwrap().get("rabbit");
        let mongo_channel = self.channels.as_ref().unwrap().get("mongo");

        let tm_block_mysql_sync = self.tm_block_mysql_sync.unwrap();
        let tm_tx_mysql_sync = self.tm_tx_mysql_sync.unwrap();
        let tm_block_mongo_sync = self.tm_block_mongo_sync.unwrap();
        let tm_tx_mongo_sync = self.tm_tx_mongo_sync.unwrap();
        let tm_block_publish = self.tm_block_publish.unwrap();
        let tm_tx_publish = self.tm_tx_publish.unwrap();
        let app = app::quit_handle().unwrap();

        let schema = self.schema.as_ref().unwrap().clone();
        app::spawn_blocking(move || {
            loop {
                if app.is_quiting() {
                    break;
                }
                let sub_events_try_lock = sub_events.try_lock();
                if sub_events_try_lock.is_none() {
                    continue;
                }
                let mut sub_events_lock = sub_events_try_lock.unwrap();
                if let Ok(msg) = monitor.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let method = TendermintMethod::find(get_str(parsed_msg, "method").unwrap()).unwrap();
                    let params = get_object(parsed_msg, "value").unwrap();
                    match method {
                        TendermintMethod::Subscribe => {
                            let new_event = SubscribeEvent::new(CHAIN, &params);
                            sub_events_lock.insert(new_event.task_id.clone(), new_event.clone());

                            let task = SubscribeTask::from(&new_event, String::from(""));
                            let msg = RocksMsg::new(RocksMethod::Put, new_event.task_id, Value::String(json!(task).to_string()));
                            let _ = rocks_channel.send(msg);
                        }
                        TendermintMethod::Unsubscribe => {
                            let task_id = get_string(&params, "task_id").unwrap();
                            sub_events_lock.remove(&task_id);

                            let msg = RocksMsg::new(RocksMethod::Delete, task_id, Value::Null);
                            let _ = rocks_channel.send(msg);
                        }
                        TendermintMethod::Resubscribe => {
                            let task_id = get_str(&params, "task_id").unwrap();
                            let mut sub_event = sub_events_lock.get(task_id).unwrap().clone();
                            sub_event.node_idx = 0;
                            sub_event.status = SubscribeStatus::Working;
                            sub_events_lock.insert(sub_event.task_id.clone(), sub_event.clone());

                            let task = SubscribeTask::from(&sub_event, String::from(""));
                            let msg = RocksMsg::new(RocksMethod::Put, sub_event.task_id, Value::String(json!(task).to_string()));
                            let _ = rocks_channel.send(msg);
                        }
                        TendermintMethod::Stop => {
                            let task_id = get_str(&params, "task_id").unwrap();
                            let mut sub_event = sub_events_lock.get(task_id).unwrap().clone();
                            sub_event.status = SubscribeStatus::Stopped;
                            sub_events_lock.insert(sub_event.task_id.clone(), sub_event.clone());

                            let task = SubscribeTask::from(&sub_event, String::from(""));
                            let msg = RocksMsg::new(RocksMethod::Put, sub_event.task_id, Value::String(json!(task).to_string()));
                            let _ = rocks_channel.send(msg);
                        }
                    };
                }

                for (_, sub_event) in sub_events_lock.iter_mut() {
                    if sub_event.is_workable() {
                        match sub_event.target {
                            SubscribeTarget::Block => {
                                let node_index = usize::from(sub_event.node_idx);
                                let req_url = format!("{}/blocks/{}", sub_event.nodes[node_index], sub_event.curr_height);
                                let response = request::get(req_url.as_ref());

                                match response {
                                    Ok(body) => {
                                        let block_result = get_object(&body, "block");
                                        let block = match block_result {
                                            Ok(block) => block,
                                            Err(err) => {
                                                println!("{}", err);
                                                continue;
                                            }
                                        };
                                        let header = block.get("header").unwrap();
                                        let header_object = header.as_object().unwrap();

                                        let filtered_result = libs::serde::filter(header_object, sub_event.filter.clone());
                                        if filtered_result.is_err() {
                                            let err = filtered_result.unwrap_err();
                                            Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                            continue;
                                        } else if filtered_result.is_ok() && !filtered_result.unwrap() {
                                            continue;
                                        }

                                        println!("event_id={}, header={}", sub_event.event_id(), header.to_string());

                                        if tm_block_mysql_sync {
                                            if let Err(err) = Self::mysql_send(
                                                &mysql_channel,
                                                schema.get("tm_block").unwrap().clone(),
                                                header.as_object().unwrap(),
                                            ) {
                                                println!("{}", err);
                                            }
                                        }
                                        if tm_block_mongo_sync {
                                            let mongo_msg = MongoMsg::new(String::from("tm_block"), header.clone());
                                            let _ = mongo_channel.send(mongo_msg);
                                        }
                                        if tm_block_publish {
                                            let _ = rabbit_channel.send(Value::String(header.to_string()));
                                        }

                                        Self::sync_event(&rocks_channel, sub_event);
                                        sub_event.curr_height += 1;
                                    }
                                    Err(err) => {
                                        if err.to_string().starts_with("requested block height") {
                                            println!("waiting for next block...");
                                        } else {
                                            Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        }
                                    }
                                }
                            }
                            SubscribeTarget::Tx => {
                                let node_idx = usize::from(sub_event.node_idx);
                                let latest_req_url = format!("{}/blocks/latest", sub_event.nodes[node_idx]);
                                let response = request::get(latest_req_url.as_ref());
                                match response {
                                    Ok(body) => {
                                        let height_result = get_value_by_path(&body, "block.header.height");
                                        let latest_height = match height_result {
                                            Ok(height) => u64::from_str(height.as_str().unwrap()).unwrap(),
                                            Err(err) => {
                                                Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                                continue;
                                            }
                                        };
                                        if sub_event.curr_height > latest_height {
                                            println!("waiting for next block...");
                                            continue;
                                        }
                                        let node_index = usize::from(sub_event.node_idx);
                                        let req_url = format!("{}/cosmos/tx/v1beta1/txs?events=tx.height={}", sub_event.nodes[node_index], sub_event.curr_height);
                                        let response = request::get(req_url.as_ref());
                                        match response {
                                            Ok(body) => {
                                                let txs_result = get_array(&body, "tx_responses");
                                                let txs = match txs_result {
                                                    Ok(txs) => txs,
                                                    Err(err) => {
                                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                                        continue;
                                                    }
                                                };
                                                for tx in txs.iter() {
                                                    let tx_object = tx.as_object().unwrap();
                                                    let filtered_result = libs::serde::filter(tx_object, sub_event.filter.clone());
                                                    if filtered_result.is_err() {
                                                        let err = filtered_result.unwrap_err();
                                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                                        continue;
                                                    } else if filtered_result.is_ok() && !filtered_result.unwrap() {
                                                        continue;
                                                    }

                                                    println!("event_id={}, tx={}", sub_event.event_id(), tx.to_string());

                                                    if tm_tx_mysql_sync {
                                                        if let Err(err) = Self::mysql_send(
                                                            &mysql_channel,
                                                            schema.get("tm_tx").unwrap().clone(),
                                                            tx_object,
                                                        ) {
                                                            println!("{}", err);
                                                        }
                                                    }
                                                    if tm_tx_mongo_sync {
                                                        let mongo_msg = MongoMsg::new(String::from("tm_tx"), tx.clone());
                                                        if let Err(err) = mongo_channel.send(mongo_msg) {
                                                            println!("{}", err);
                                                        }
                                                    }
                                                    if tm_tx_publish {
                                                        if let Err(err) = rabbit_channel.send(Value::String(tx.to_string())) {
                                                            println!("{}", err);
                                                        }
                                                    }
                                                }

                                                Self::sync_event(&rocks_channel, sub_event);
                                                sub_event.curr_height += 1;
                                            }
                                            Err(err) => {
                                                Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    fn shutdown(&mut self) {}
}

impl TendermintPlugin {
    fn init(&mut self) {
        self.sub_events = Some(Arc::new(FutureMutex::new(HashMap::new())));
        let channels = MultiChannel::new(vec!("tendermint", "rocks", "mysql", "rabbit", "mongo"));
        self.channels = Some(channels.to_owned());
        self.monitor = Some(app::subscribe_channel(String::from("tendermint")));
        self.schema = Some(HashMap::new());
        self.tm_block_mysql_sync = Some(libs::opts::bool("tendermint::block-mysql-sync").unwrap());
        self.tm_tx_mysql_sync = Some(libs::opts::bool("tendermint::tx-mysql-sync").unwrap());
        self.tm_block_mongo_sync = Some(libs::opts::bool("tendermint::block-mongo-sync").unwrap());
        self.tm_tx_mongo_sync = Some(libs::opts::bool("tendermint::tx-mongo-sync").unwrap());
        self.tm_block_publish = Some(libs::opts::bool("tendermint::block-rabbit-mq-publish").unwrap());
        self.tm_tx_publish = Some(libs::opts::bool("tendermint::tx-rabbit-mq-publish").unwrap());
    }

    fn init_mysql(&mut self) {
        if let Some(state) = app::plugin_state::<MySqlPlugin>() {
            if state != State::Initialized {
                return;
            }
        }
        let json_str = fs::read_to_string("schema/mysql.json").unwrap();
        let json_schema: Value = serde_json::from_str(json_str.as_str()).unwrap();
        let schema_map = json_schema.as_object().unwrap();

        let schema = self.schema.as_mut().unwrap();
        for (table, values) in schema_map {
            let created_schema = Schema::from(table.clone(), values).unwrap();
            schema.insert(table.clone(), created_schema);
        }

        let plugin_handle = app::get_plugin::<MySqlPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let mysql = plugin.downcast_mut::<MySqlPlugin>().unwrap();

        for (_, selected_schema) in schema.iter() {
            let result = mysql.execute(selected_schema.create_table.clone(), mysql::Params::Empty);
            if let Err(err) = result {
                println!("error={}", err.to_string());
            }
        }

        let plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let jsonrpc = plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let pool = mysql.get_pool();
        jsonrpc.add_method(String::from("tm_mysql_get_blocks"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_blocks::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }
            let order = get_str(&params, "order").unwrap();
            let query = format!("select * from tm_block where height >= :from_height and height <= :to_height order by 1 {}", order);
            let picked_params = select_value(&params, vec!["from_height", "to_height"]).unwrap();
            let result = MySqlPlugin::query_static(&pool, query, get_params(&picked_params)).unwrap();
            Box::new(futures::future::ok(Value::Array(result)))
        });

        let pool = mysql.get_pool();
        jsonrpc.add_method(String::from("tm_mysql_get_txs"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_txs::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }
            let (query, picked_params) = if params.get("txhash").is_some() {
                let query = String::from("select * from tm_tx where txhash=:txhash");
                let picked_params = select_value(&params, vec!["txhash"]).unwrap();
                (query, picked_params)
            } else {
                let order = get_str(&params, "order").unwrap();
                let query = format!("select * from tm_tx where height >= :from_height and height <= :to_height order by 1 {}", order);
                let picked_params = select_value(&params, vec!["from_height", "to_height"]).unwrap();
                (query, picked_params)
            };
            let result = MySqlPlugin::query_static(&pool, query, get_params(&picked_params)).unwrap();
            Box::new(futures::future::ok(Value::Array(result)))
        });
    }

    fn register_jsonrpc(&self) {
        let plugin_handle = app::get_plugin::<JsonRpcPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let jsonrpc = plugin.downcast_mut::<JsonRpcPlugin>().unwrap();

        let plugin_handle = app::get_plugin::<RocksPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let rocks = plugin.downcast_mut::<RocksPlugin>().unwrap();

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_subscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = subscribe::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }
            let task_id = SubscribeTask::task_id(CHAIN, &params);
            let value = get_static(&rocks_db, task_id.as_str());
            if value.is_null() {
                let message = TendermintMsg::new(TendermintMethod::Subscribe, Value::Object(params.clone()));
                let _ = tm_channel.send(message);

                Box::new(futures::future::ok(Value::String(format!("subscription requested! task_id={}", task_id))))
            } else {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(format!("already exist task! task_id={}", task_id)));
                Box::new(futures::future::ok(Value::Object(error)))
            }
        });

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_unsubscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }

            let task_id = get_str(&params, "task_id").unwrap();
            let value = get_static(&rocks_db, task_id);
            if value.is_null() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(format!("task does not exist! task_id={}", task_id)));
                Box::new(futures::future::ok(Value::Object(error)))
            } else {
                let tm_msg = TendermintMsg::new(TendermintMethod::Unsubscribe, Value::Object(params.clone()));
                let _ = tm_channel.send(tm_msg);

                Box::new(futures::future::ok(Value::String(format!("unsubscription requested! task_id={}", task_id))))
            }
        });

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_resubscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = resubscribe::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }

            let task_id = get_str(&params, "task_id").unwrap();
            let value = get_static(&rocks_db, task_id);
            if value.is_null() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(format!("subscription does not exist! task_id={}", task_id)));
                Box::new(futures::future::ok(Value::Object(error)))
            } else {
                let message = TendermintMsg::new(TendermintMethod::Resubscribe, Value::Object(params.clone()));
                let _ = tm_channel.send(message);

                Box::new(futures::future::ok(Value::String(format!("resubscription requested! task_id={}", task_id))))
            }
        });

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_stop_subscription"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = stop_subscribe::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }

            let task_id = get_str(&params, "task_id").unwrap();
            let value = get_static(&rocks_db, task_id);
            if value.is_null() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(format!("task does not exist! task_id={}", task_id)));
                Box::new(futures::future::ok(Value::Object(error)))
            } else {
                let tm_msg = TendermintMsg::new(TendermintMethod::Stop, Value::Object(params.clone()));
                let _ = tm_channel.send(tm_msg);

                Box::new(futures::future::ok(Value::String(format!("stop subscription requested! task_id={}", task_id))))
            }
        });

        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_get_tasks"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_task::verify(&params);
            if verified.is_err() {
                let mut error = Map::new();
                error.insert(String::from("error"), Value::String(verified.unwrap_err().to_string()));
                return Box::new(futures::future::ok(Value::Object(error)));
            }

            let prefix = match params.get("task_id") {
                None => TASK_PREFIX,
                Some(task_id) => task_id.as_str().unwrap(),
            };
            let tasks = get_by_prefix_static(&rocks_db, prefix);
            Box::new(futures::future::ok(tasks))
        });
    }

    fn load_tasks(&self) {
        let plugin_handle = app::get_plugin::<RocksPlugin>();
        let mut plugin = plugin_handle.lock().unwrap();
        let rocks = plugin.downcast_mut::<RocksPlugin>().unwrap();

        let rocks_db = rocks.get_db();
        let raw_tasks = get_by_prefix_static(&rocks_db, TASK_PREFIX);
        let sub_events = Arc::clone(self.sub_events.as_ref().unwrap());
        raw_tasks.as_array().unwrap().iter()
            .for_each(|raw_task| {
                let task = raw_task.as_object().unwrap();
                let event = SubscribeEvent::from(task);
                let mut sub_events_lock = sub_events.try_lock().unwrap();
                sub_events_lock.insert(event.task_id.clone(), event);
            });
    }

    fn sync_event(rocks_channel: &channel::Sender, sub_event: &mut SubscribeEvent) {
        let task = SubscribeTask::from(&sub_event, String::from(""));
        let task_id = task.task_id.clone();

        let msg = RocksMsg::new(RocksMethod::Put, task_id, Value::String(json!(task).to_string()));
        let _ = rocks_channel.send(msg);
    }

    fn error_handler(rocks_channel: &channel::Sender, sub_event: &mut SubscribeEvent, err_msg: String) {
        sub_event.handle_err(err_msg.clone());
        let task = SubscribeTask::from(sub_event, err_msg);

        let msg = RocksMsg::new(RocksMethod::Put, sub_event.task_id.clone(), Value::String(json!(task).to_string()));
        let _ = rocks_channel.send(msg);
    }

    fn mysql_send(mysql_channel: &channel::Sender, schema: Schema, values: &Map<String, Value>) -> Result<(), ExpectedError> {
        let insert_query = schema.insert_query;
        let names: Vec<&str> = schema.attributes.iter().map(|attribute| { attribute.name.as_str() }).collect();
        let picked_value = select_value(values, names)?;
        let mysql_msg = MySqlMsg::new(String::from(insert_query), Value::Object(picked_value));
        let _ = mysql_channel.send(mysql_msg)?;
        Ok(())
    }
}
