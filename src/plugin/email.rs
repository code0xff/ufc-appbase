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
            Err(err) => println!("could not send email: {:?}", err),
        }
    }
}

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
        EmailPlugin::recv(monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl EmailPlugin {
    fn recv(monitor: SubscribeHandle, app: QuitHandle) {
        tokio::spawn(async move {
            loop {
                if let Some(mut mon_lock) = monitor.try_lock() {
                    if let Ok(msg) = mon_lock.try_recv() {
                        let parsed_msg = msg.as_object().unwrap();
                        let parsed_to = get_str(parsed_msg, "to");
                        if parsed_to.is_err() {
                            println!("{}", parsed_to.clone().unwrap_err());
                            break;
                        }
                        let to = parsed_to.unwrap();

                        let parsed_subject = get_str(parsed_msg, "subject");
                        if parsed_subject.is_err() {
                            println!("{}", parsed_subject.clone().unwrap_err());
                            break;
                        }
                        let subject = parsed_subject.unwrap();

                        let parsed_body = get_str(parsed_msg, "body");
                        if parsed_body.is_err() {
                            println!("{}", parsed_body.clone().unwrap_err());
                            break;
                        }
                        let body = parsed_body.unwrap();

                        Self::send(to, subject, body);
                    }
                }
                break;
            }
            if !app.is_quiting() {
                EmailPlugin::recv(monitor, app);
            }
        });
    }
}