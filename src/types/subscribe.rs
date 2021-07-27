use appbase::ChannelHandle;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{enumeration, get_str, get_string, get_string_vec, get_u64};
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
    pub status: SubscribeStatus,
}

impl SubscribeEvent {
    pub fn new(chain: String, params: &Map<String, Value>) -> SubscribeEvent {
        let sub_id = get_string!(params; "sub_id");
        let start_height = get_u64!(params; "start_height");
        SubscribeEvent {
            task_id: format!("task:{}:{}:{}", chain, get_str!(params; "target"), sub_id),
            target: SubscribeTarget::find(get_str!(params; "target")).unwrap(),
            chain,
            sub_id,
            start_height,
            curr_height: start_height,
            nodes: get_string_vec!(params; "nodes"),
            node_idx: 0,
            status: SubscribeStatus::Working,
        }
    }

    pub fn from(params: &Map<String, Value>) -> SubscribeEvent {
        SubscribeEvent {
            task_id: get_string!(params; "task_id"),
            target: SubscribeTarget::find(get_str!(params; "target")).unwrap(),
            chain: get_string!(params; "chain"),
            sub_id: get_string!(params; "sub_id"),
            start_height: get_u64!(params; "start_height"),
            curr_height: get_u64!(params; "curr_height"),
            nodes: get_string_vec!(params; "nodes"),
            node_idx: 0,
            status: SubscribeStatus::find(get_str!(params; "status")).unwrap(),
        }
    }

    pub fn is_workable(&self) -> bool {
        vec!(Working).contains(&self.status)
    }

    pub fn event_id(&self) -> String {
        format!("{}:{}:{}:{}", self.chain, self.target.value(), self.sub_id, self.curr_height)
    }

    pub fn handle_err(&mut self, rocks_channel: &ChannelHandle, err_msg: String) {
        println!("{}", err_msg);
        if usize::from(self.node_idx) + 1 < self.nodes.len() {
            self.node_idx += 1;
        } else {
            self.err(rocks_channel, err_msg);
        }
    }

    pub fn err(&mut self, rocks_channel: &ChannelHandle, err_msg: String) {
        println!("{}", err_msg);
        self.status = SubscribeStatus::Error;
        let task = SubscribeTask::err(self, err_msg);
        let msg = RocksMsg::new(RocksMethod::Put, self.task_id.clone(), Some(Value::String(json!(task).to_string())));
        let _ = rocks_channel.lock().unwrap().send(msg);
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
    pub fn err(sub_block: &SubscribeEvent, err_msg: String) -> SubscribeTask {
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

    pub fn from(sub_block: &SubscribeEvent) -> SubscribeTask {
        SubscribeTask {
            task_id: sub_block.task_id.clone(),
            target: sub_block.target.value(),
            chain: sub_block.chain.clone(),
            sub_id: sub_block.sub_id.clone(),
            start_height: sub_block.start_height,
            curr_height: sub_block.curr_height,
            nodes: sub_block.nodes.clone(),
            status: sub_block.status.value(),
            err_msg: String::from(""),
        }
    }

    pub fn task_id(chain: &str, params: &Map<String, Value>) -> String {
        format!("task:{}:{}:{}", chain, get_str!(params; "target"), get_str!(params; "sub_id"))
    }
}

enumeration!(SubscribeTarget; {Block: "block"}, {Tx: "tx"});
enumeration!(SubscribeStatus; {Working: "working"}, {Error: "error"});

impl SubscribeTarget {

}
