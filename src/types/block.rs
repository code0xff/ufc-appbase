use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::types::block::SubscribeStatus::{Requested, Working};

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
            status: SubscribeStatus::Requested,
        }
    }

    pub fn from(params: &Map<String, Value>) -> SubscribeBlock {
        let start_height = params.get("start_height").unwrap().as_u64().unwrap();
        let nodes = params.get("nodes").unwrap().as_array().unwrap().iter().map(|n| { String::from(n.as_str().unwrap()) }).collect();
        SubscribeBlock {
            task_id: String::from(params.get("task_id").unwrap().as_str().unwrap()),
            chain: String::from(params.get("chain").unwrap().as_str().unwrap()),
            chain_id: String::from(params.get("chain_id").unwrap().as_str().unwrap()),
            start_height,
            curr_height: start_height,
            nodes,
            node_idx: 0,
            status: SubscribeStatus::Requested
        }
    }

    pub fn request_url(&self) -> String {
        let node_index = usize::from(self.node_idx);
        self.nodes[node_index].to_string() + self.curr_height.to_string().as_str()
    }

    pub fn is_workable(&self) -> bool {
        vec!(Requested, Working).contains(&self.status)
    }

    pub fn block_id(&self) -> String {
        format!("{}:{}:{}", self.chain, self.chain_id, self.curr_height)
    }

    pub fn handle_err(&mut self) {
        if usize::from(self.node_idx) + 1 < self.nodes.len() {
            self.node_idx += 1;
        } else {
            self.status = SubscribeStatus::Error
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BlockTask {
    pub task_id: String,
    pub chain: String,
    pub chain_id: String,
    pub start_height: u64,
    pub nodes: Vec<String>,
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
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum SubscribeStatus {
    Requested,
    Working,
    Error,
}
