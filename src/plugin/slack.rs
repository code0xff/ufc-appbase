use std::collections::HashMap;
use std::sync::Arc;

use appbase::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::libs::serde::get_str;
use crate::message;

pub struct SlackPlugin {
    base: PluginBase,
    monitor: Option<SubscribeHandle>,
}

message!(SlackMsg; {slack_hook: String}, {msg: String});

appbase_plugin_requires!(SlackPlugin; );

impl SlackPlugin {}

impl Plugin for SlackPlugin {
    appbase_plugin_default!(SlackPlugin);

    fn new() -> Self {
        SlackPlugin {
            base: PluginBase::new(),
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        self.monitor = Some(app::subscribe_channel(String::from("slack")));
    }

    fn startup(&mut self) {
        if !self.plugin_startup() {
            return;
        }
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let slack_hook = get_str(parsed_msg, "slack_hook");
                    if slack_hook.is_err() {
                        println!("{}", slack_hook.unwrap_err());
                        continue;
                    }

                    let slack_msg = get_str(parsed_msg, "msg");
                    if slack_msg.is_err() {
                        println!("{}", slack_msg.unwrap_err());
                        continue;
                    }
                    let mut text = HashMap::new();
                    text.insert("text", slack_msg.unwrap());

                    let client = reqwest::Client::new();
                    let post_result = client.post(slack_hook.unwrap())
                        .json(&text)
                        .send()
                        .await;
                    match post_result {
                        Ok(res) => {
                            println!("{:?}", res);
                        }
                        Err(err) => {
                            println!("{:?}", err);
                        }
                    }
                }
            }
        });
    }

    fn shutdown(&mut self) {
        if !self.plugin_shutdown() {
            return;
        }
    }
}
