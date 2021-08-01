use std::sync::Arc;

use appbase::*;
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::libs;
use crate::message;
use crate::libs::serde::get_str;

pub struct EmailPlugin {
    base: PluginBase,
    monitor: Option<SubscribeHandle>,
}

message!(EmailMsg; {to: String}, {subject: String}, {body: String});

appbase_plugin_requires!(EmailPlugin; );

impl EmailPlugin {
    pub fn send(to: &str, subject: &str, body: &str) {
        let smtp_username = libs::environment::string("SMTP_USERNAME").unwrap();
        let smtp_password = libs::environment::string("SMTP_PASSWORD").unwrap();
        let credentials = Credentials::new(smtp_username, smtp_password);
        let smtp_relay = libs::environment::string("SMTP_RELAY").unwrap();
        let from = libs::environment::string("EMAIL_FROM").unwrap();
        let reply_to = libs::environment::string("EMAIL_REPLY_TO").unwrap();

        let email = Message::builder()
            .from(from.as_str().parse().unwrap())
            .reply_to(reply_to.as_str().parse().unwrap())
            .to(to.parse().unwrap())
            .subject(subject)
            .body(String::from(body))
            .unwrap();

        let mailer = SmtpTransport::relay(smtp_relay.as_str())
            .unwrap()
            .credentials(credentials)
            .build();

        match mailer.send(&email) {
            Ok(_) => println!("email sent successfully!"),
            Err(e) => panic!("could not send email: {:?}", e),
        }
    }
}

impl Plugin for EmailPlugin {
    appbase_plugin_default!(EmailPlugin);

    fn new() -> Self {
        EmailPlugin {
            base: PluginBase::new(),
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        if !self.plugin_initialize() {
            return;
        }
        self.monitor = Some(app::subscribe_channel(String::from("email")));
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
                    let parsed_to = get_str(parsed_msg, "to");
                    if parsed_to.is_err() {
                        println!("{}", parsed_to.clone().unwrap_err());
                    }
                    let to = parsed_to.unwrap();

                    let parsed_subject = get_str(parsed_msg, "subject");
                    if parsed_subject.is_err() {
                        println!("{}", parsed_subject.clone().unwrap_err());
                    }
                    let subject = parsed_subject.unwrap();

                    let parsed_body = get_str(parsed_msg, "body");
                    if parsed_body.is_err() {
                        println!("{}", parsed_body.clone().unwrap_err());
                    }
                    let body = parsed_body.unwrap();

                    Self::send(to, subject, body);
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
