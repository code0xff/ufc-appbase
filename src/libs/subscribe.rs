use appbase::channel;
use appbase::channel::Sender;
use serde_json::{Map, Value};

use crate::error::error::ExpectedError;
use crate::libs;
use crate::libs::serde::select_value;
use crate::plugin::mongo::MongoMsg;
use crate::plugin::mysql::MySqlMsg;
use crate::types::mysql::Schema;

pub fn callback(prefix: String, value: &Value, schema: &Schema, mysql: &Sender, mongo: &Sender, rabbit: &Sender) -> Result<(), ExpectedError> {
    if libs::opts::bool(format!("{}-mysql-sync", prefix).as_str())? {
        mysql_send(&mysql, schema, value.as_object().unwrap())?
    }
    if libs::opts::bool(format!("{}-mongo-sync", prefix).as_str())? {
        let mongo_msg = MongoMsg::new(String::from(schema.table.clone()), value.clone());
        let _ = mongo.send(mongo_msg)?;
    }
    if libs::opts::bool(format!("{}-rabbit-mq-publish", prefix).as_str())? {
        let _ = rabbit.send(Value::String(value.to_string()))?;
    }
    Ok(())
}

fn mysql_send(mysql_channel: &channel::Sender, schema: &Schema, values: &Map<String, Value>) -> Result<(), ExpectedError> {
    let insert_query = schema.insert_query.clone();
    let names: Vec<&str> = schema.attributes.iter().map(|attribute| { attribute.name.as_str() }).collect();
    let picked_value = select_value(values, names)?;
    let mysql_msg = MySqlMsg::new(insert_query, Value::Object(picked_value));
    let _ = mysql_channel.send(mysql_msg)?;
    Ok(())
}