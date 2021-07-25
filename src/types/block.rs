use appbase::ChannelHandle;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::plugin::rocks::{RocksMethod, RocksMsg};
use crate::types::block::SubscribeStatus::Working;

#[derive(Debug, Clone)]
pub struct SubscribeBlock {
    pub task_id: String,
    pub chain: String,
    pub chain_id: String,
    pub start_height: u64,
    pub curr_height: u64,
    pub nodes: Vec<String>,
    pub node_idx: u16,
    pub status: SubscribeStatus,
}

impl SubscribeBlock {
    pub fn new(chain: String, params: &Map<String, Value>) -> SubscribeBlock {
        let nodes = params.get("nodes").unwrap().as_array().unwrap().iter().map(|n| { String::from(n.as_str().unwrap()) }).collect();
        let chain_id = String::from(params.get("chain_id").unwrap().as_str().unwrap());
        let start_height = params.get("start_height").unwrap().as_u64().unwrap();
        SubscribeBlock {
            task_id: format!("{}:{}:{}", "task:block", chain, chain_id),
            chain,
            chain_id,
            start_height,
            curr_height: start_height,
            nodes,
            node_idx: 0,
            status: SubscribeStatus::Working,
        }
    }

    pub fn from(params: &Map<String, Value>) -> SubscribeBlock {
        let start_height = params.get("start_height").unwrap().as_u64().unwrap();
        let nodes = params.get("nodes").unwrap().as_array().unwrap().iter().map(|n| { String::from(n.as_str().unwrap()) }).collect();
        let task_status = params.get("status").unwrap().as_str().unwrap().to_string();
        SubscribeBlock {
            task_id: String::from(params.get("task_id").unwrap().as_str().unwrap()),
            chain: String::from(params.get("chain").unwrap().as_str().unwrap()),
            chain_id: String::from(params.get("chain_id").unwrap().as_str().unwrap()),
            start_height,
            curr_height: start_height,
            nodes,
            node_idx: 0,
            status: BlockTask::sub_status(task_status),
        }
    }

    pub fn request_url(&self) -> String {
        let node_index = usize::from(self.node_idx);
        self.nodes[node_index].to_string() + self.curr_height.to_string().as_str()
    }

    pub fn is_workable(&self) -> bool {
        vec!(Working).contains(&self.status)
    }

    pub fn block_id(&self) -> String {
        format!("{}:{}:{}", self.chain, self.chain_id, self.curr_height)
    }

    pub fn handle_err(&mut self, rocks_ch: &ChannelHandle, err_msg: String) {
        println!("{}", err_msg);
        if usize::from(self.node_idx) + 1 < self.nodes.len() {
            self.node_idx += 1;
        } else {
            self.err(rocks_ch, err_msg);
        }
    }

    pub fn err(&mut self, rocks_channel: &ChannelHandle, err_msg: String) {
        println!("{}", err_msg);
        self.status = SubscribeStatus::Error;
        let task = BlockTask::from(self, err_msg);
        let task_json = json!(task);
        let msg = RocksMsg::new(RocksMethod::Put, self.task_id.clone(), Some(Value::String(task_json.to_string())));
        let _ = rocks_channel.lock().unwrap().send(msg);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BlockTask {
    pub task_id: String,
    pub chain: String,
    pub chain_id: String,
    pub start_height: u64,
    pub nodes: Vec<String>,
    pub status: String,
    pub err_msg: String,
}

impl BlockTask {
    pub fn new(chain: String, params: &Map<String, Value>) -> BlockTask {
        let nodes = params.get("nodes").unwrap().as_array().unwrap().iter().map(|n| { String::from(n.as_str().unwrap()) }).collect();
        BlockTask {
            task_id: format!("{}:{}:{}", "task:block", chain, params.get("chain_id").unwrap().as_str().unwrap()),
            chain,
            chain_id: String::from(params.get("chain_id").unwrap().as_str().unwrap()),
            start_height: params.get("start_height").unwrap().as_u64().unwrap(),
            nodes,
            status: SubscribeStatus::Working.value(),
            err_msg: String::from(""),
        }
    }

    pub fn from(sub_block: &SubscribeBlock, err_msg: String) -> BlockTask {
        BlockTask {
            task_id: sub_block.task_id.clone(),
            chain: sub_block.chain.clone(),
            chain_id: sub_block.chain_id.clone(),
            start_height: sub_block.start_height,
            nodes: sub_block.nodes.clone(),
            status: sub_block.status.value(),
            err_msg,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum SubscribeStatus {
    Working,
    Error,
}

impl SubscribeStatus {
    fn value(&self) -> String {
        match self {
            SubscribeStatus::Working => String::from("working"),
            SubscribeStatus::Error => String::from("error"),
        }
    }

    fn find(method: &str) -> SubscribeStatus {
        match method {
            "working" => SubscribeStatus::Working,
            "error" => SubscribeStatus::Error,
            _ => {
                panic!("matched status does not exist");
            }
        }
    }
}