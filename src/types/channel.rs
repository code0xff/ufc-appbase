use std::collections::HashMap;
use appbase::{ChannelHandle, app};
use std::sync::Arc;

#[derive(Clone)]
pub struct MultiChannel {
    channel_map: HashMap<String, ChannelHandle>,
}

impl MultiChannel {
    pub fn new(channels: Vec<String>) -> Self {
        let mut channel_map = HashMap::new();
        for channel_nm in channels.iter() {
            channel_map.insert(channel_nm.clone(), app::get_channel(channel_nm.clone()));
        }
        MultiChannel {
            channel_map: channel_map.to_owned(),
        }
    }

    // pub fn add(&mut self, name: String) {
    //     self.channel_map.insert(name.clone(), app::get_channel(name.clone()));
    // }

    pub fn get(&self, name: &str) -> ChannelHandle {
        Arc::clone(self.channel_map.get(name).unwrap())
    }
}
