use std::collections::HashMap;

use appbase::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::libs;
use crate::libs::serde::get_str;
use crate::message;

pub struct TelegramPlugin {
    token: Option<String>,
    monitor: Option<SubscribeHandle>,
}

message!(TelegramMsg; {chat_id: String}, {text: String});

appbase_plugin_requires!(TelegramPlugin; );

impl Plugin for TelegramPlugin {
    fn new() -> Self {
        TelegramPlugin {
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
        let monitor = self.monitor.as_ref().unwrap().clone();
        let token = self.token.as_ref().unwrap().clone();
        let app = app::quit_handle().unwrap();
        Self::recv(monitor, token, app);
    }

    fn shutdown(&mut self) {}
}

impl TelegramPlugin {
    fn recv(monitor: SubscribeHandle, token: String, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();
                    let chat_id = get_str(parsed_msg, "chat_id").unwrap();
                    let text = get_str(parsed_msg, "text").unwrap();

                    let mut req_body = HashMap::new();
                    req_body.insert("chat_id", chat_id);
                    req_body.insert("text", text);

                    let client = reqwest::Client::new();
                    let result = client.post(format!("https://api.telegram.org/bot{}/sendMessage", token))
                        .json(&req_body)
                        .send()
                        .await;
                    if let Err(err) = result {
                        println!("telegram_error={:?}", err);
                    }
                };
            }
            if !app.is_quiting() {
                Self::recv(monitor, token, app);
            }
        });
    }
}
