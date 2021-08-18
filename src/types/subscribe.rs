use appbase::channel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::enumeration;
use crate::libs::serde::{get_str, get_string, get_string_vec, get_u64};
use crate::plugin::rocks::{RocksMethod, RocksMsg};
use crate::types::enumeration::Enumeration;
use crate::types::subscribe::SubscribeStatus::Working;

#[derive(Debug, Clone)]
pub struct SubscribeEvent {
    pub task_id: String,
    pub target: SubscribeTarget,
    pub chain: String,
    pub sub_id: String,
    pub start_height: u64,
    pub curr_height: u64,
    pub nodes: Vec<String>,
    pub node_idx: u16,
    pub filter: String,
    pub status: SubscribeStatus,
}

impl SubscribeEvent {
    pub fn new(chain: &str, params: &Map<String, Value>) -> Self {
        let sub_id = get_string(params, "sub_id").unwrap();
        let start_height = get_u64(params, "start_height").unwrap();
        let target = get_str(params, "target").unwrap();
        let filter_result = get_string(params, "filter");
        let filter = match filter_result {
            Ok(filter) => filter,
            Err(_) => String::from("")
        };
        SubscribeEvent {
            task_id: format!("task:{}:{}:{}", chain, target, sub_id),
            target: SubscribeTarget::find(target).unwrap(),
            chain: String::from(chain),
            sub_id,
            start_height,
            curr_height: start_height,
            nodes: get_string_vec(params, "nodes"),
            node_idx: 0,
            filter,
            status: SubscribeStatus::Working,
        }
    }

    pub fn from(params: &Map<String, Value>) -> Self {
        SubscribeEvent {
            task_id: get_string(params, "task_id").unwrap(),
            target: SubscribeTarget::find(get_str(params, "target").unwrap()).unwrap(),
            chain: get_string(params, "chain").unwrap(),
            sub_id: get_string(params, "sub_id").unwrap(),
            start_height: get_u64(params, "start_height").unwrap(),
            curr_height: get_u64(params, "curr_height").unwrap(),
            nodes: get_string_vec(params, "nodes"),
            node_idx: get_u64(params, "node_idx").unwrap() as u16,
            filter: get_string(params, "filter").unwrap(),
            status: SubscribeStatus::find(get_str(params, "status").unwrap()).unwrap(),
        }
    }

    pub fn is_workable(&self) -> bool {
        vec!(Working).contains(&self.status)
    }

    pub fn event_id(&self) -> String {
        format!("{}:{}:{}:{}", self.chain, self.target.value(), self.sub_id, self.curr_height)
    }

    pub fn handle_error(&mut self, rocks_channel: &channel::Sender, err_msg: String) {
        println!("{}", err_msg.clone());
        if usize::from(self.node_idx) + 1 < self.nodes.len() {
            self.node_idx += 1;
        } else {
            self.status = SubscribeStatus::Error;
        }
        let task = SubscribeTask::from(self, err_msg.clone());
        let msg = RocksMsg::new(RocksMethod::Put, self.task_id.clone(), Value::String(json!(task).to_string()));
        let _ = rocks_channel.send(msg);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SubscribeTask {
    pub task_id: String,
    pub target: String,
    pub chain: String,
    pub sub_id: String,
    pub start_height: u64,
    pub curr_height: u64,
    pub nodes: Vec<String>,
    pub node_idx: u16,
    pub filter: String,
    pub status: String,
    pub err_msg: String,
}

impl SubscribeTask {
    pub fn from(sub_event: &SubscribeEvent, err_msg: String) -> Self {
        SubscribeTask {
            task_id: sub_event.task_id.clone(),
            target: sub_event.target.value(),
            chain: sub_event.chain.clone(),
            sub_id: sub_event.sub_id.clone(),
            start_height: sub_event.start_height,
            curr_height: sub_event.curr_height,
            nodes: sub_event.nodes.clone(),
            node_idx: sub_event.node_idx,
            filter: sub_event.filter.clone(),
            status: sub_event.status.value(),
            err_msg,
        }
    }

    pub fn task_id(chain: &str, params: &Map<String, Value>) -> String {
        format!("task:{}:{}:{}", chain, get_str(params, "target").unwrap(), get_str(params, "sub_id").unwrap())
    }
}

enumeration!(SubscribeTarget; {Block: "block"}, {Tx: "tx"});
enumeration!(SubscribeStatus; {Working: "working"}, {Stopped: "stopped"}, {Error: "error"});

#[cfg(test)]
mod subscribe_test {
    use appbase::*;
    use serde_json::{json, Map, Value};

    use crate::types::subscribe::{SubscribeEvent, SubscribeStatus};

    #[test]
    fn subscribe_event_task_id_test() {
        let mut params = Map::new();
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1u64));
        params.insert(String::from("target"), json!("block"));
        params.insert(String::from("nodes"), json!(["https://api.cosmos.network"]));
        params.insert(String::from("filter"), Value::String(String::from("")));

        let subscribe_event = SubscribeEvent::new("tendermint", &params);
        assert_eq!(subscribe_event.task_id, "task:tendermint:block:cosmoshub-4");
    }

    #[test]
    fn subscribe_event_is_workable_test() {
        let mut params = Map::new();
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1u64));
        params.insert(String::from("target"), json!("block"));
        params.insert(String::from("nodes"), json!(["https://api.cosmos.network"]));
        params.insert(String::from("filter"), Value::String(String::from("")));

        let subscribe_event = SubscribeEvent::new("tendermint", &params);
        assert!(subscribe_event.is_workable());
    }

    #[test]
    fn subscribe_event_event_id_test() {
        let mut params = Map::new();
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1u64));
        params.insert(String::from("target"), json!("block"));
        params.insert(String::from("nodes"), json!(["https://api.cosmos.network"]));
        params.insert(String::from("filter"), Value::String(String::from("")));

        let subscribe_event = SubscribeEvent::new("tendermint", &params);
        assert_eq!(subscribe_event.event_id(), "tendermint:block:cosmoshub-4:1");
    }
}
