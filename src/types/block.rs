#[derive(Debug, Clone)]
pub struct SubscribeBlock {
    pub chain: String,
    pub chain_id: String,
    pub start_height: u64,
    pub current_height: u64,
    pub nodes: Vec<String>,
    pub node_index: u16,
    pub status: SubscribeStatus,
}

#[derive(Debug, Clone)]
pub struct BlockTask {
    pub chain: String,
    pub chain_id: String,
    pub start_height: u64,
    pub nodes: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SubscribeStatus {
    Requested,
    Working,
    RequestError,
    ServerError,
}
