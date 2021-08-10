use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::enumeration;
use crate::libs::serde::{get_str, get_string, get_string_vec, get_u64};
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
    pub status: SubscribeStatus,
}

impl SubscribeEvent {
    pub fn new(chain: &str, params: &Map<String, Value>) -> Self {
        let sub_id = get_string(params, "sub_id").unwrap();
        let start_height = get_u64(params, "start_height").unwrap();
        let target = get_str(params, "target").unwrap();
        SubscribeEvent {
            task_id: format!("task:{}:{}:{}", chain, target, sub_id),
            target: SubscribeTarget::find(target).unwrap(),
            chain: String::from(chain),
            sub_id,
            start_height,
            curr_height: start_height,
            nodes: get_string_vec(params, "nodes"),
            node_idx: 0,
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
            node_idx: 0,
            status: SubscribeStatus::find(get_str(params, "status").unwrap()).unwrap(),
        }
    }

    pub fn is_workable(&self) -> bool {
        vec!(Working).contains(&self.status)
    }

    pub fn event_id(&self) -> String {
        format!("{}:{}:{}:{}", self.chain, self.target.value(), self.sub_id, self.curr_height)
    }

    pub fn handle_err(&mut self, err_msg: String) {
        println!("{}", err_msg);
        if usize::from(self.node_idx) + 1 < self.nodes.len() {
            self.node_idx += 1;
        } else {
            self.err(err_msg);
        }
    }

    pub fn err(&mut self, err_msg: String) {
        println!("{}", err_msg);
        self.status = SubscribeStatus::Error;
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
    pub status: String,
    pub err_msg: String,
}

impl SubscribeTask {
    pub fn from(sub_block: &SubscribeEvent, err_msg: String) -> Self {
        SubscribeTask {
            task_id: sub_block.task_id.clone(),
            target: sub_block.target.value(),
            chain: sub_block.chain.clone(),
            sub_id: sub_block.sub_id.clone(),
            start_height: sub_block.start_height,
            curr_height: sub_block.curr_height,
            nodes: sub_block.nodes.clone(),
            status: sub_block.status.value(),
            err_msg,
        }
    }

    pub fn task_id(chain: &str, params: &Map<String, Value>) -> String {
        format!("task:{}:{}:{}", chain, get_str(params, "target").unwrap(), get_str(params, "sub_id").unwrap())
    }
}

enumeration!(SubscribeTarget; {Block: "block"}, {Tx: "tx"});
enumeration!(SubscribeStatus; {Working: "working"}, {Error: "error"});

#[cfg(test)]
mod subscribe_test {
    use appbase::*;
    use serde_json::{json, Map};

    use crate::types::subscribe::{SubscribeEvent, SubscribeStatus};

    #[test]
    fn subscribe_event_task_id_test() {
        let mut params = Map::new();
        params.insert(String::from("sub_id"), json!("cosmoshub-4"));
        params.insert(String::from("start_height"), json!(1u64));
        params.insert(String::from("target"), json!("block"));
        params.insert(String::from("nodes"), json!(["https://api.cosmos.network"]));

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

        let subscribe_event = SubscribeEvent::new("tendermint", &params);
        assert_eq!(subscribe_event.event_id(), "tendermint:block:cosmoshub-4:1");
    }
}
