use std::{fs, thread};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use appbase::*;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{enumeration, libs, message};
use crate::libs::rocks::{get_by_prefix_static, get_static};
use crate::libs::serde::{get_array, get_object, get_str, get_string, pick};
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mongo::{MongoMsg, MongoPlugin};
use crate::plugin::mysql::{MySqlMsg, MySqlPlugin};
use crate::plugin::rocks::{RocksMethod, RocksMsg, RocksPlugin};
use crate::types::channel::MultiChannel;
use crate::types::enumeration::Enumeration;
use crate::types::mysql::Schema;
use crate::types::subscribe::{SubscribeEvent, SubscribeTarget, SubscribeTask};
use crate::validation::{get_task, subscribe, unsubscribe};

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

message!((TendermintMsg; {value: Value}); (TendermintMethod; {Subscribe: "subscribe"}, {Unsubscribe: "unsubscribe"}));

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
        // self.load_schema();
        self.load_tasks();
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
        tokio::task::spawn_blocking(move || {
            loop {
                if app.is_quiting() {
                    break;
                }
                thread::sleep(Duration::from_secs(1));
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
                    };
                }

                for (_, sub_event) in sub_events_lock.iter_mut() {
                    if sub_event.is_workable() {
                        match sub_event.target {
                            SubscribeTarget::Block => {
                                let node_index = usize::from(sub_event.node_idx);
                                let req_url = format!("{}/blocks/{}", sub_event.nodes[node_index], sub_event.curr_height);
                                let res_result = reqwest::blocking::get(req_url);

                                let res = match res_result {
                                    Ok(res) => res,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };

                                let status = res.status().clone();
                                let res_body = res.text();
                                let body_str = match res_body {
                                    Ok(body_str) => body_str,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };

                                let parsed_body = serde_json::from_str(body_str.as_str());
                                let body: Map<String, Value> = match parsed_body {
                                    Ok(body) => body,
                                    Err(err) => {
                                        println!("parsing error occurred... error={:?}", err);
                                        continue;
                                    }
                                };
                                if !status.is_success() {
                                    let err_msg = get_string(&body, "error").unwrap();
                                    if err_msg.starts_with("requested block height") {
                                        println!("waiting for next block...");
                                    } else {
                                        Self::error_handler(&rocks_channel, sub_event, err_msg);
                                    }
                                    continue;
                                }

                                let block_result = get_object(&body, "block");
                                let block = match block_result {
                                    Ok(block) => block,
                                    Err(err) => {
                                        println!("{}", err);
                                        continue;
                                    }
                                };
                                let header = block.get("header").unwrap();

                                println!("event_id={}, header={}", sub_event.event_id(), header.to_string());

                                if tm_block_mysql_sync {
                                    let header_object = header.as_object().unwrap();
                                    let selected_schema = schema.get("tm_block").unwrap().clone();
                                    let insert_query = selected_schema.insert_query;
                                    let values = pick(header_object, vec!["chain_id", "height", "time", "last_commit_hash", "data_hash", "validators_hash", "next_validators_hash", "consensus_hash", "app_hash", "last_results_hash", "evidence_hash", "proposer_address"]);
                                    if values.is_ok() {
                                        let mysql_msg = MySqlMsg::new(String::from(insert_query), Value::Object(values.unwrap()));
                                        let _ = mysql_channel.send(mysql_msg);
                                    } else {
                                        println!("{}", values.unwrap_err());
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
                            SubscribeTarget::Tx => {
                                let node_idx = usize::from(sub_event.node_idx);
                                let latest_req_url = format!("{}/blocks/latest", sub_event.nodes[node_idx]);
                                let res_result = reqwest::blocking::get(latest_req_url);
                                let res = match res_result {
                                    Ok(res) => res,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };
                                let res_body = res.text();
                                let body: Map<String, Value> = match res_body {
                                    Ok(body) => serde_json::from_str(body.as_str()).unwrap(),
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };

                                let block_result = get_object(&body, "block");
                                let block = match block_result {
                                    Ok(block) => block,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };
                                let header_result = get_object(block, "header");
                                let header = match header_result {
                                    Ok(header) => header,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };
                                let height_result = get_str(header, "height");
                                let last_height = match height_result {
                                    Ok(height) => u64::from_str(height).unwrap(),
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };
                                if sub_event.curr_height > last_height {
                                    println!("waiting for next block...");
                                    continue;
                                }
                                let node_index = usize::from(sub_event.node_idx);

                                let req_url = format!("{}/cosmos/tx/v1beta1/txs?events=tx.height={}", sub_event.nodes[node_index], sub_event.curr_height);
                                let res_result = reqwest::blocking::get(req_url);

                                let res = match res_result {
                                    Ok(res) => res,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };
                                let status = res.status().clone();
                                let res_body = res.text();

                                let body: Map<String, Value> = match res_body {
                                    Ok(body) => serde_json::from_str(body.as_str()).unwrap(),
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };

                                if !status.is_success() {
                                    let err_msg = get_string(&body, "error").unwrap();
                                    Self::error_handler(&rocks_channel, sub_event, err_msg);
                                    continue;
                                }

                                let txs_result = get_array(&body, "tx_responses");
                                let txs = match txs_result {
                                    Ok(txs) => txs,
                                    Err(err) => {
                                        Self::error_handler(&rocks_channel, sub_event, err.to_string());
                                        continue;
                                    }
                                };
                                for tx in txs.iter() {
                                    println!("event_id={}, tx={}", sub_event.event_id(), tx.to_string());

                                    if tm_tx_mysql_sync {
                                        let tx_object = tx.as_object().unwrap();
                                        let selected_schema = schema.get("tm_tx").unwrap().clone();
                                        let insert_query = selected_schema.insert_query;

                                        let values = pick(tx_object, vec!["txhash", "height", "gas_wanted", "gas_used", "raw_log", "timestamp"]);
                                        if values.is_ok() {
                                            let mysql_msg = MySqlMsg::new(String::from(insert_query), Value::Object(values.unwrap()));
                                            let _ = mysql_channel.send(mysql_msg);
                                        } else {
                                            println!("{}", values.unwrap_err());
                                        }
                                    }
                                    if tm_tx_mongo_sync {
                                        let mongo_msg = MongoMsg::new(String::from("tm_tx"), tx.clone());
                                        let _ = mongo_channel.send(mongo_msg);
                                    }
                                    if tm_tx_publish {
                                        let _ = rabbit_channel.send(Value::String(tx.to_string()));
                                    }
                                }

                                Self::sync_event(&rocks_channel, sub_event);
                                sub_event.curr_height += 1;
                                break;
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
        self.tm_tx_publish = Some(libs::opts::bool("tendermint::tx-rabiit-mq-publish").unwrap());
    }

    fn load_schema(&mut self) {
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
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }
            let message = TendermintMsg::new(TendermintMethod::Subscribe, Value::Object(params.clone()));
            let _ = tm_channel.send(message);
            let task_id = SubscribeTask::task_id(CHAIN, &params);

            let value = get_static(&rocks_db, task_id.as_str());
            if value.is_null() {
                let task_id = SubscribeTask::task_id(CHAIN, &params);
                Box::new(futures::future::ready(Ok(Value::String(format!("subscription requested! task_id={}", task_id)))))
            } else {
                Box::new(futures::future::ready(Ok(Value::String(format!("already exist task! task_id={}", task_id)))))
            }
        });

        let tm_channel = self.channels.as_ref().unwrap().get("tendermint");
        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_unsubscribe"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = unsubscribe::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let tm_msg = TendermintMsg::new(TendermintMethod::Unsubscribe, Value::Object(params.clone()));
            let _ = tm_channel.send(tm_msg);

            let task_id = get_str(&params, "task_id").unwrap();
            let value = get_static(&rocks_db, task_id);
            if value.is_null() {
                Box::new(futures::future::ready(Ok(Value::String(format!("task does not exist! task_id={}", task_id)))))
            } else {
                Box::new(futures::future::ready(Ok(Value::String(format!("unsubscription requested! task_id={}", task_id)))))
            }
        });

        let rocks_db = rocks.get_db();
        jsonrpc.add_method(String::from("tm_get_tasks"), move |params: Params| {
            let params: Map<String, Value> = params.parse().unwrap();
            let verified = get_task::verify(&params);
            if verified.is_err() {
                return Box::new(futures::future::ready(Ok(Value::String(verified.unwrap_err()))));
            }

            let prefix = match params.get("task_id") {
                None => TASK_PREFIX,
                Some(task_id) => task_id.as_str().unwrap(),
            };
            let tasks = get_by_prefix_static(&rocks_db, prefix);
            Box::new(futures::future::ready(Ok(tasks)))
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
                if event.is_workable() {
                    let mut sub_events_lock = sub_events.try_lock().unwrap();
                    sub_events_lock.insert(event.task_id.clone(), event);
                }
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
}
