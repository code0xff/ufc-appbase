#[derive(Clone)]
pub struct SubscribeBlock {
    pub chain: String,
    pub chain_id: String,
    pub start_height: u64,
    pub current_height: u64,
    pub nodes: Vec<String>,
    pub node_index: i32,
}
