use std::collections::HashMap;

use appbase::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::libs::serde::get_str;
use crate::message;

pub struct SlackPlugin {
    monitor: Option<SubscribeHandle>,
}

message!(SlackMsg; {slack_hook: String}, {msg: String});

appbase_plugin_requires!(SlackPlugin; );

impl Plugin for SlackPlugin {
    fn new() -> Self {
        SlackPlugin {
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        self.monitor = Some(app::subscribe_channel(String::from("slack")));
    }

    fn startup(&mut self) {
        let monitor = self.monitor.as_ref().unwrap().clone();
        let app = app::quit_handle().unwrap();
        Self::recv(monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl SlackPlugin {
    fn recv(monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();

                    let slack_hook = get_str(parsed_msg, "slack_hook").unwrap();
                    let slack_msg = get_str(parsed_msg, "msg").unwrap();

                    let mut text = HashMap::new();
                    text.insert("text", slack_msg);

                    let client = reqwest::Client::new();
                    let result = client.post(slack_hook)
                        .json(&text)
                        .send()
                        .await;
                    if let Err(err) = result {
                        println!("slack error={:?}", err);
                    }
                }
            }
            if !app.is_quiting() {
                Self::recv(monitor, app);
            }
        });
    }
}
