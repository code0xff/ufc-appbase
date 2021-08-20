use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use appbase::*;
use appbase::channel::Sender;
use appbase::plugin::State;
use futures::lock::Mutex as FutureMutex;
use jsonrpc_core::Params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{enumeration, libs, message};
use crate::error::error::ExpectedError;
use crate::libs::mysql::get_params;
use crate::libs::opts::opt_to_result;
use crate::libs::request;
use crate::libs::rocks::{get_by_prefix_static, get_static};
use crate::libs::serde::{get_array, get_object, get_str, get_string, get_value_by_path, select_value};
use crate::plugin::jsonrpc::JsonRpcPlugin;
use crate::plugin::mysql::MySqlPlugin;
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

        let mut rocks_channel = self.channels.as_ref().unwrap().get("rocks");
        let mysql_channel = self.channels.as_ref().unwrap().get("mysql");
        let rabbit_channel = self.channels.as_ref().unwrap().get("rabbit");
        let mongo_channel = self.channels.as_ref().unwrap().get("mongo");

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
                    Self::message_handler(&msg, &mut sub_events_lock, &mut rocks_channel);
                }

                for (_, sub_event) in sub_events_lock.iter_mut() {
                    if !sub_event.is_workable() {
                        continue;
                    }
                    if sub_event.target == SubscribeTarget::Block {
                        let block_header = Self::poll_block_header(sub_event);
                        match block_header {
                            Ok(header) => {
                                println!("event_id={}, header={}", sub_event.event_id(), header.to_string());

                                let schema_opt = schema.get("tm_block");
                                let prefix = format!("{}::{}", CHAIN, sub_event.target.value());
                                if let Err(err) = libs::callback::mysql(prefix.clone(), &header, schema_opt, &mysql_channel) {
                                    println!("{}", err.to_string());
                                };
                                if let Err(err) = libs::callback::mongo(prefix.clone(), &header, "tm_block", &mongo_channel) {
                                    println!("{}", err.to_string());
                                };
                                if let Err(err) = libs::callback::rabbit(prefix, &header, &rabbit_channel) {
                                    println!("{}", err.to_string());
                                };

                                Self::sync_event(&rocks_channel, sub_event);
                                sub_event.curr_height += 1;
                            }
                            Err(error) => Self::error_handler(&rocks_channel, sub_event, error)
                        };
                    } else {
                        let block_txs_result = Self::poll_block_txs(sub_event);
                        match block_txs_result {
                            Ok(block_txs) => {
                                if block_txs.len() > 0 {
                                    let txs_result = Self::poll_txs(sub_event);
                                    let txs = match txs_result {
                                        Ok(txs) => txs,
                                        Err(err) => {
                                            sub_event.handle_error(&rocks_channel, err.to_string());
                                            continue;
                                        }
                                    };

                                    for tx in txs.iter() {
                                        let tx_object = tx.as_object().unwrap();
                                        let filtered_result = libs::serde::filter(tx_object, sub_event.filter.clone());
                                        if filtered_result.is_err() {
                                            let err = filtered_result.unwrap_err();
                                            sub_event.handle_error(&rocks_channel, err.to_string());
                                            continue;
                                        } else if filtered_result.is_ok() && !filtered_result.unwrap() {
                                            continue;
                                        }

                                        println!("event_id={}, tx={}", sub_event.event_id(), tx.to_string());

                                        let schema_opt = schema.get("tm_tx");
                                        let prefix = format!("{}::{}", CHAIN, sub_event.target.value());
                                        if let Err(err) = libs::callback::mysql(prefix.clone(), &tx, schema_opt, &mysql_channel) {
                                            println!("{}", err.to_string());
                                        };
                                        if let Err(err) = libs::callback::mongo(prefix.clone(), &tx, "tm_tx", &mongo_channel) {
                                            println!("{}", err.to_string());
                                        };
                                        if let Err(err) = libs::callback::rabbit(prefix, &tx, &rabbit_channel) {
                                            println!("{}", err.to_string());
                                        };
                                    }
                                } else {
                                    println!("block txs is empty! curr_height={}", sub_event.curr_height);
                                }
                                Self::sync_event(&rocks_channel, sub_event);
                                sub_event.curr_height += 1;
                            }
                            Err(error) => Self::error_handler(&rocks_channel, sub_event, error)
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
    }

    fn init_mysql(&mut self) {
        if let Some(state) = app::plugin_state::<MySqlPlugin>() {
            if state != State::Initialized {
                return;
            }
        }
        let json_str = fs::read_to_string("schema/tm_mysql.json").unwrap();
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
            let selected_params = select_value(&params, vec!["from_height", "to_height"]).unwrap();
            let result = MySqlPlugin::query_static(&pool, query, get_params(&selected_params)).unwrap();
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
            let (query, selected_params) = if params.get("txhash").is_some() {
                let query = String::from("select * from tm_tx where txhash=:txhash");
                let selected_params = select_value(&params, vec!["txhash"]).unwrap();
                (query, selected_params)
            } else {
                let order = get_str(&params, "order").unwrap();
                let query = format!("select * from tm_tx where height >= :from_height and height <= :to_height order by 1 {}", order);
                let selected_params = select_value(&params, vec!["from_height", "to_height"]).unwrap();
                (query, selected_params)
            };
            let result = MySqlPlugin::query_static(&pool, query, get_params(&selected_params)).unwrap();
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
        let mut sub_events_lock = sub_events.try_lock().unwrap();
        for raw_task in raw_tasks.as_array().unwrap() {
            let task = raw_task.as_object().unwrap();
            let event = SubscribeEvent::from(task);
            sub_events_lock.insert(event.task_id.clone(), event);
        }
    }

    fn sync_event(rocks_channel: &channel::Sender, sub_event: &mut SubscribeEvent) {
        let task = SubscribeTask::from(&sub_event, String::from(""));
        let task_id = task.task_id.clone();

        let msg = RocksMsg::new(RocksMethod::Put, task_id, Value::String(json!(task).to_string()));
        let _ = rocks_channel.send(msg);
    }

    fn error_handler(rocks_channel: &channel::Sender, sub_event: &mut SubscribeEvent, error: ExpectedError) {
        match error {
            ExpectedError::BlockHeightError(err_msg) => println!("{}", err_msg),
            ExpectedError::FilterError(err_msg) => {
                println!("{}", err_msg);
                Self::sync_event(&rocks_channel, sub_event);
                sub_event.curr_height += 1;
            }
            _ => {
                sub_event.handle_error(&rocks_channel, error.to_string());
            }
        };
    }

    fn message_handler(msg: &Value, sub_events: &mut HashMap<String, SubscribeEvent>, rocks_channel: &mut Sender) {
        let parsed_msg = msg.as_object().unwrap();
        let method = TendermintMethod::find(get_str(parsed_msg, "method").unwrap()).unwrap();
        let params = get_object(parsed_msg, "value").unwrap();
        match method {
            TendermintMethod::Subscribe => {
                let new_event = SubscribeEvent::new(CHAIN, &params);
                sub_events.insert(new_event.task_id.clone(), new_event.clone());

                let task = SubscribeTask::from(&new_event, String::from(""));
                let msg = RocksMsg::new(RocksMethod::Put, new_event.task_id, Value::String(json!(task).to_string()));
                let _ = rocks_channel.send(msg);
            }
            TendermintMethod::Unsubscribe => {
                let task_id = get_string(&params, "task_id").unwrap();
                sub_events.remove(&task_id);

                let msg = RocksMsg::new(RocksMethod::Delete, task_id, Value::Null);
                let _ = rocks_channel.send(msg);
            }
            TendermintMethod::Resubscribe => {
                let task_id = get_str(&params, "task_id").unwrap();
                let mut sub_event = sub_events.get(task_id).unwrap().clone();
                sub_event.node_idx = 0;
                sub_event.status = SubscribeStatus::Working;
                sub_events.insert(sub_event.task_id.clone(), sub_event.clone());

                let task = SubscribeTask::from(&sub_event, String::from(""));
                let msg = RocksMsg::new(RocksMethod::Put, sub_event.task_id, Value::String(json!(task).to_string()));
                let _ = rocks_channel.send(msg);
            }
            TendermintMethod::Stop => {
                let task_id = get_str(&params, "task_id").unwrap();
                let mut sub_event = sub_events.get(task_id).unwrap().clone();
                sub_event.status = SubscribeStatus::Stopped;
                sub_events.insert(sub_event.task_id.clone(), sub_event.clone());

                let task = SubscribeTask::from(&sub_event, String::from(""));
                let msg = RocksMsg::new(RocksMethod::Put, sub_event.task_id, Value::String(json!(task).to_string()));
                let _ = rocks_channel.send(msg);
            }
        };
    }

    fn poll_block(sub_event: &mut SubscribeEvent) -> Result<Value, ExpectedError> {
        let node_index = usize::from(sub_event.node_idx);
        let req_url = format!("{}/blocks/{}", sub_event.nodes[node_index], sub_event.curr_height);
        let response = request::get(req_url.as_str());
        let body = match response {
            Ok(body) => body,
            Err(err) => {
                return if err.to_string().starts_with("requested block height") {
                    Err(ExpectedError::BlockHeightError(err.to_string()))
                } else {
                    Err(err)
                };
            }
        };
        let block = opt_to_result(body.get("block"))?;
        Ok(block.clone())
    }

    fn poll_block_header(sub_event: &mut SubscribeEvent) -> Result<Value, ExpectedError> {
        let block_value = Self::poll_block(sub_event)?;
        let block = opt_to_result(block_value.as_object())?;
        let header = opt_to_result(block.get("header"))?;
        let header_object = opt_to_result(header.as_object())?;

        let filter_result = libs::serde::filter(header_object, sub_event.filter.clone())?;
        if !filter_result {
            return Err(ExpectedError::FilterError(String::from("value does not match on filter condition!")));
        }
        Ok(header.clone())
    }

    fn poll_block_txs(sub_event: &mut SubscribeEvent) -> Result<Vec<Value>, ExpectedError> {
        let block_value = Self::poll_block(sub_event)?;
        let block = opt_to_result(block_value.as_object())?;
        let txs_value = get_value_by_path(block, "data.txs")?;
        if !txs_value.is_array() {
            return Err(ExpectedError::TypeError(String::from("txs in block data is not array!")));
        }
        let txs = opt_to_result(txs_value.as_array())?;

        Ok(txs.clone())
    }

    fn poll_txs(sub_event: &mut SubscribeEvent) -> Result<Vec<Value>, ExpectedError> {
        let node_index = usize::from(sub_event.node_idx);
        let req_url = format!("{}/cosmos/tx/v1beta1/txs?events=tx.height={}", sub_event.nodes[node_index], sub_event.curr_height);
        let body = request::get(req_url.as_str())?;

        let txs_result = get_array(&body, "tx_responses")?;
        Ok(txs_result.clone())
    }
}
