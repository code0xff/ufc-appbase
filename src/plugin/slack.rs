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

impl SlackPlugin {}

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
        SlackPlugin::recv(monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl SlackPlugin {
    fn recv(monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            loop {
                if let Some(mut mon_lock) = monitor.try_lock() {
                    if let Ok(msg) = mon_lock.try_recv() {
                        let parsed_msg = msg.as_object().unwrap();
                        let slack_hook = get_str(parsed_msg, "slack_hook");
                        if slack_hook.is_err() {
                            println!("{}", slack_hook.unwrap_err());
                            break;
                        }

                        let slack_msg = get_str(parsed_msg, "msg");
                        if slack_msg.is_err() {
                            println!("{}", slack_msg.unwrap_err());
                            break;
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
                break;
            }
            if !app.is_quiting() {
                SlackPlugin::recv(monitor, app);
            }
        });
    }
}
