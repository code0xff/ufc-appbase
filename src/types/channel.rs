use std::collections::HashMap;
use appbase::{channel, app};

#[derive(Clone)]
pub struct MultiChannel {
    channel_map: HashMap<String, channel::Sender>,
}

impl MultiChannel {
    pub fn new(channels: Vec<&str>) -> Self {
        let mut channel_map = HashMap::new();
        for channel in channels.into_iter() {
            channel_map.insert(String::from(channel), app::get_channel(String::from(channel)));
        }
        MultiChannel {
            channel_map: channel_map.to_owned(),
        }
    }

    // pub fn add(&mut self, name: String) {
    //     self.channel_map.insert(name.clone(), app::get_channel(name.clone()));
    // }

    pub fn get(&self, name: &str) -> channel::Sender {
        self.channel_map.get(name).unwrap().clone()
    }
}
