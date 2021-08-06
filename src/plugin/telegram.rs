use std::collections::HashMap;
use std::sync::{Arc};

use appbase::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::libs;
use crate::libs::serde::{get_str};
use crate::message;

pub struct TelegramPlugin {
    base: PluginBase,
    token: Option<String>,
    monitor: Option<SubscribeHandle>,
}

message!(TelegramMsg; {chat_id: String}, {text: String});

appbase_plugin_requires!(TelegramPlugin; );

impl TelegramPlugin {}

impl Plugin for TelegramPlugin {
    appbase_plugin_default!(TelegramPlugin);

    fn new() -> Self {
        TelegramPlugin {
            base: PluginBase::new(),
            token: None,
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        let token = libs::environment::string("TELEGRAM_BOT_TOKEN").unwrap();
        self.token = Some(token.clone());
        self.monitor = Some(app::subscribe_channel(String::from("telegram")));
    }

    fn startup(&mut self) {
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let token = self.token.as_ref().unwrap().clone();
        tokio::spawn(async move {
            let mut mon_lock = monitor.lock().await;
            loop {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let chat_id = get_str(parsed_msg, "chat_id");
                    if chat_id.is_err() {
                        println!("{}", chat_id.unwrap_err());
                        continue;
                    }

                    let text = get_str(parsed_msg, "text");
                    if text.is_err() {
                        println!("{}", text.unwrap_err());
                        continue;
                    }
                    let mut req_body = HashMap::new();
                    req_body.insert("chat_id", chat_id.unwrap());
                    req_body.insert("text", text.unwrap());

                    let client = reqwest::Client::new();
                    let post_result = client.post(format!("https://api.telegram.org/bot{}/sendMessage", token))
                        .json(&req_body)
                        .send()
                        .await;
                    match post_result {
                        Ok(res) => {
                            println!("{:?}", res);
                        }
                        Err(err) => {
                            println!("error={:?}", err);
                        }
                    }
                }
            }
        });
    }

    fn shutdown(&mut self) {}
}
