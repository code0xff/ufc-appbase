use appbase::ChannelHandle;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::plugin::rocks::{RocksMethod, RocksMsg};
use crate::types::subscribe::SubscribeStatus::Working;
use crate::types::enumeration::Enumeration;

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
        let sub_id = String::from(params.get("sub_id").unwrap().as_str().unwrap());
        let start_height = params.get("start_height").unwrap().as_u64().unwrap();
        SubscribeEvent {
            task_id: format!("task:{}:{}:{}", chain, params.get("target").unwrap().as_str().unwrap(), sub_id),
            target: SubscribeTarget::find(params.get("target").unwrap().as_str().unwrap()).unwrap(),
            chain,
            sub_id,
            start_height,
            curr_height: start_height,
            nodes: params.get("nodes").unwrap().as_array().unwrap().iter().map(|n| { String::from(n.as_str().unwrap()) }).collect(),
            node_idx: 0,
            status: SubscribeStatus::Working,
        }
    }

    pub fn from(params: &Map<String, Value>) -> SubscribeEvent {
        SubscribeEvent {
            task_id: String::from(params.get("task_id").unwrap().as_str().unwrap()),
            target: SubscribeTarget::find(params.get("target").unwrap().as_str().unwrap()).unwrap(),
            chain: String::from(params.get("chain").unwrap().as_str().unwrap()),
            sub_id: String::from(params.get("sub_id").unwrap().as_str().unwrap()),
            start_height: params.get("start_height").unwrap().as_u64().unwrap(),
            curr_height: params.get("curr_height").unwrap().as_u64().unwrap(),
            nodes: params.get("nodes").unwrap().as_array().unwrap().iter().map(|n| { String::from(n.as_str().unwrap()) }).collect(),
            node_idx: 0,
            status: SubscribeStatus::find(params.get("status").unwrap().as_str().unwrap()).unwrap(),
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
        let task_json = json!(task);
        let msg = RocksMsg::new(RocksMethod::Put, self.task_id.clone(), Some(Value::String(task_json.to_string())));
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
        format!("task:{}:{}:{}", chain, params.get("target").unwrap().as_str().unwrap(), params.get("sub_id").unwrap().as_str().unwrap())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum SubscribeTarget {
    Block,
    Tx,
}

impl Enumeration for SubscribeTarget {
    fn value(&self) -> String {
        match self {
            SubscribeTarget::Block => String::from("block"),
            SubscribeTarget::Tx => String::from("tx"),
        }
    }

    fn find(target: &str) -> Option<Self> {
        match target {
            "block" => Some(SubscribeTarget::Block),
            "tx" => Some(SubscribeTarget::Tx),
            _ => None,
        }
    }
}

impl SubscribeTarget {
    pub fn valid(target: &str) -> bool {
        Self::find(target).is_some()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum SubscribeStatus {
    Working,
    Error,
}

impl Enumeration for SubscribeStatus {
    fn value(&self) -> String {
        match self {
            SubscribeStatus::Working => String::from("working"),
            SubscribeStatus::Error => String::from("error"),
        }
    }

    fn find(name: &str) -> Option<Self> {
        match name {
            "working" => Some(SubscribeStatus::Working),
            "error" => Some(SubscribeStatus::Error),
            _ => None,
        }
    }
}
