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
    monitor: Option<channel::Receiver>,
}

message!(EmailMsg; {to: String}, {subject: String}, {body: String});

plugin::requires!(EmailPlugin; );

impl Plugin for EmailPlugin {
    fn new() -> Self {
        app::arg(clap::Arg::new("email::smtp-username").long("smtp-username").takes_value(true));
        app::arg(clap::Arg::new("email::smtp-password").long("smtp-password").takes_value(true));
        app::arg(clap::Arg::new("email::smtp-relay").long("smtp-relay").takes_value(true));
        app::arg(clap::Arg::new("email::from").long("email-from").takes_value(true));
        app::arg(clap::Arg::new("email::reply-to").long("email-reply-to").takes_value(true));

        EmailPlugin {
            monitor: None,
        }
    }

    fn initialize(&mut self) {
        self.monitor = Some(app::subscribe_channel(String::from("email")));
    }

    fn startup(&mut self) {
        let monitor = self.monitor.take().unwrap();
        let app = app::quit_handle().unwrap();
        Self::recv(monitor, app);
    }

    fn shutdown(&mut self) {}
}

impl EmailPlugin {
    fn recv(mut monitor: channel::Receiver, app: QuitHandle) {
        app::spawn(async move {
            if let Ok(msg) = monitor.try_recv() {
                let parsed_msg = msg.as_object().unwrap();

                let to = get_str(parsed_msg, "to").unwrap();
                let subject = get_str(parsed_msg, "subject").unwrap();
                let body = get_str(parsed_msg, "body").unwrap();

                if let Err(result) = Self::send(to, subject, body) {
                    println!("{}", result);
                }
            }
            if !app.is_quiting() {
                Self::recv(monitor, app);
            }
        });
    }

    pub fn send(to: &str, subject: &str, body: &str) -> Result<(), ExpectedError> {
        let smtp_username = libs::opts::string("email::smtp-username")?;
        let smtp_password = libs::opts::string("email::smtp-password")?;
        let credentials = Credentials::new(smtp_username, smtp_password);
        let smtp_relay = libs::opts::string("email::smtp-relay")?;
        let from = libs::opts::string("email::from")?;
        let reply_to = libs::opts::string("email::reply-to")?;

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
