use std::sync::Arc;

use appbase::*;
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::error::ExpectedError;
use crate::libs;
use crate::libs::serde::get_str;
use crate::message;

pub struct EmailPlugin {
    monitor: Option<SubscribeHandle>,
}

message!(EmailMsg; {to: String}, {subject: String}, {body: String});

appbase_plugin_requires!(EmailPlugin; );

impl Plugin for EmailPlugin {
    fn new() -> Self {
        EmailPlugin {
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        self.monitor = Some(app::subscribe_channel(String::from("email")));
    }

    fn startup(&mut self) {
        let monitor = Arc::clone(self.monitor.as_ref().unwrap());
        let app = app::quit_handle().unwrap();
        Self::recv(monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl EmailPlugin {
    fn recv(monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            if let Some(mut mon_lock) = monitor.try_lock() {
                if let Ok(msg) = mon_lock.try_recv() {
                    let parsed_msg = msg.as_object().unwrap();

                    let to = get_str(parsed_msg, "to").unwrap();
                    let subject = get_str(parsed_msg, "subject").unwrap();
                    let body = get_str(parsed_msg, "body").unwrap();

                    if let Err(result) = Self::send(to, subject, body) {
                        println!("{}", result);
                    }
                }
            }
            if !app.is_quiting() {
                Self::recv(monitor, app);
            }
        });
    }

    pub fn send(to: &str, subject: &str, body: &str) -> Result<(), ExpectedError> {
        let smtp_username = libs::environment::string("SMTP_USERNAME")?;
        let smtp_password = libs::environment::string("SMTP_PASSWORD")?;
        let credentials = Credentials::new(smtp_username, smtp_password);
        let smtp_relay = libs::environment::string("SMTP_RELAY")?;
        let from = libs::environment::string("EMAIL_FROM")?;
        let reply_to = libs::environment::string("EMAIL_REPLY_TO")?;

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

        let _ = mailer.send(&email)?;
        Ok(())
    }
}
