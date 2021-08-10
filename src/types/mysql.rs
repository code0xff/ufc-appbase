use std::fmt::Debug;

use jsonrpc_core::Value;

use crate::enumeration;
use crate::error::error::ExpectedError;
use crate::libs::serde::{get_array, get_bool, get_object, get_string};
use crate::types::enumeration::Enumeration;

#[derive(Clone, Debug)]
pub struct Schema {
    table: String,
    attributes: Vec<Attribute>,
    pub create_table: String,
    pub insert_query: String,
}

#[derive(Clone, Debug)]
struct Attribute {
    name: String,
    _type: String,
    size: Option<u32>,
    nullable: bool,
}

impl Schema {
    pub fn from(table: String, values: &Value) -> Result<Schema, ExpectedError> {
        if !values.is_object() {
            return Err(ExpectedError::TypeError(String::from("input values is not object type!")));
        }
        let map = values.as_object().unwrap();
        let raw_attributes = get_object(map, "attributes")?;

        let attributes = raw_attributes.iter().map(|(key, value)| {
            let parsed_value = value.as_object().unwrap();
            let size = match parsed_value.get("size") {
                None => None,
                Some(size) => Some(size.as_u64().unwrap() as u32)
            };
            Attribute {
                name: key.clone(),
                _type: get_string(parsed_value, "type").unwrap(),
                size,
                nullable: get_bool(parsed_value, "nullable").unwrap(),
            }
        }).collect();

        let uniques = get_array(map, "uniques")?;
        let indexes = get_array(map, "indexes")?;
        let create_table = Self::create_table(table.clone(), &attributes, uniques, indexes);
        let insert_query = Self::insert_query(table.clone(), &attributes);

        Ok(Schema {
            table: table.clone(),
            attributes,
            create_table,
            insert_query,
        })
    }


    fn create_table(table: String, attributes: &Vec<Attribute>, uniques: &Vec<Value>, indexes: &Vec<Value>) -> String {
        let mut query_line: Vec<String> = Vec::new();
        query_line.push(format!("`{}_id` bigint(20) not null auto_increment", table));
        for attribute in attributes.iter() {
            match attribute.size {
                None => {
                    query_line.push(format!("`{}` {} {}", attribute.name, attribute._type, Self::nullable(attribute.nullable)));
                }
                Some(size) => {
                    query_line.push(format!("`{}` {}({}) {}", attribute.name, attribute._type, size, Self::nullable(attribute.nullable)));
                }
            }
        }
        query_line.push(format!("PRIMARY KEY (`{}_id`)", table));

        for raw_keys in uniques.iter() {
            let unique_vec: Vec<String> = raw_keys.as_array().unwrap().iter().map(|v| { String::from(v.as_str().unwrap()) }).collect();
            let unique_name = format!("{}_{}_unique", table, unique_vec.join("_"));
            let unique_keys = unique_vec.join(", ");
            query_line.push(format!("unique key `{}` ({})", unique_name, unique_keys));
        }

        for raw_keys in indexes.iter() {
            let index_vec: Vec<String> = raw_keys.as_array().unwrap().iter().map(|v| { String::from(v.as_str().unwrap()) }).collect();
            let index_name = format!("{}_{}_index", table, index_vec.join("_"));
            let index_keys = index_vec.join(", ");
            query_line.push(format!("key `{}` ({}) using btree", index_name, index_keys));
        }

        let full_query = query_line.join(", ");

        format!("create table `{}` ({}) ENGINE=InnoDB AUTO_INCREMENT=1 DEFAULT CHARSET=utf8mb4", table, full_query)
    }

    fn insert_query(table: String, attributes: &Vec<Attribute>) -> String {
        let mut columns: Vec<String> = attributes.iter().map(|attribute| { attribute.clone().name.clone() }).collect();
        let column_str = columns.join(", ");
        let converted: Vec<String> = columns.iter_mut().map(|v| { format!(":{}", v) }).collect();
        let values = converted.join(", ");
        format!("insert into {} ({}) values ({})", table, column_str, values)
    }

    fn nullable(nullable: bool) -> String {
        if nullable {
            String::from("null")
        } else {
            String::from("not null")
        }
    }
}

enumeration!(Order; {Asc: "asc"}, {Desc: "desc"});
