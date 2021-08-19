use std::fmt::Debug;

use jsonrpc_core::Value;

use crate::enumeration;
use crate::error::error::ExpectedError;
use crate::libs::mysql::convert_type;
use crate::libs::serde::{get_array, get_object};
use crate::types::enumeration::Enumeration;

#[derive(Clone, Debug)]
pub struct Schema {
    pub table: String,
    pub attributes: Vec<Attribute>,
    pub create_table: String,
    pub insert_query: String,
}

#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: String,
    _type: String,
    max_length: Option<u32>,
    nullable: bool,
}

impl Schema {
    pub fn from(table: String, values: &Value) -> Result<Schema, ExpectedError> {
        if !values.is_object() {
            return Err(ExpectedError::TypeError(String::from("input values is not object type!")));
        }
        let map = values.as_object().unwrap();
        let raw_attributes = get_object(map, "attributes")?;

        let mut attributes: Vec<Attribute> = Vec::new();
        for (key, value) in raw_attributes {
            let parsed_value = value.as_object().unwrap();
            let size = match parsed_value.get("maxLength") {
                None => None,
                Some(size) => Some(size.as_u64().unwrap() as u32)
            };
            let type_value = match parsed_value.get("type") {
                None => return Err(ExpectedError::NoneError(String::from("mysql schema attribute must include type!"))),
                Some(type_value) => type_value
            };
            let (_type, nullable) = match type_value {
                Value::Array(v) => {
                    let v_str: Vec<String> = v.iter().map(|it| { String::from(it.as_str().unwrap()) }).collect();
                    if v_str.len() > 2 {
                        return Err(ExpectedError::InvalidError(String::from("type array size cannot be bigger than 2!")));
                    }
                    if v_str.len() > 1 && v_str.get(1).unwrap() != "null" {
                        return Err(ExpectedError::InvalidError(String::from("second value of types must be null!")));
                    }
                    (v_str.get(0).unwrap().clone(), true)
                }
                Value::String(v) => (v.clone(), false),
                _ => return Err(ExpectedError::TypeError(String::from("type only can be string or array!")))
            };

            let attribute = Attribute {
                name: key.clone(),
                _type,
                max_length: size,
                nullable,
            };
            attributes.push(attribute);
        }

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
            let converted_type = convert_type(attribute._type.clone(), attribute.max_length).unwrap();
            match attribute.max_length {
                None => {
                    query_line.push(format!("`{}` {} {}", attribute.name, converted_type, Self::null_or_not(attribute.nullable)));
                }
                Some(size) => {
                    query_line.push(format!("`{}` {}({}) {}", attribute.name, converted_type, size, Self::null_or_not(attribute.nullable)));
                }
            }
        }
        query_line.push(format!("PRIMARY KEY (`{}_id`)", table));

        for raw_keys in uniques.iter() {
            let unique_vec: Vec<String> = raw_keys.as_array().unwrap().iter().map(|v| { String::from(v.as_str().unwrap()) }).collect();
            let unique_name = format!("{}_{}_unique", table, unique_vec.join("_"));
            let unique_keys = unique_vec.iter().map(|v| { format!("`{}`", v) }).collect::<Vec<String>>().join(", ");
            query_line.push(format!("unique key `{}` ({})", unique_name, unique_keys));
        }

        for raw_keys in indexes.iter() {
            let index_vec: Vec<String> = raw_keys.as_array().unwrap().iter().map(|v| { String::from(v.as_str().unwrap()) }).collect();
            let index_name = format!("{}_{}_index", table, index_vec.join("_"));
            let index_keys = index_vec.iter().map(|v| { format!("`{}`", v) }).collect::<Vec<String>>().join(", ");
            query_line.push(format!("key `{}` ({}) using btree", index_name, index_keys));
        }

        let full_query = query_line.join(", ");

        format!("create table `{}` ({}) ENGINE=InnoDB AUTO_INCREMENT=1 DEFAULT CHARSET=utf8mb4", table, full_query)
    }

    fn insert_query(table: String, attributes: &Vec<Attribute>) -> String {
        let columns: Vec<String> = attributes.iter().map(|attribute| { attribute.clone().name.clone() }).collect();
        let column_names = columns.iter().map(|v| { format!("`{}`", v) }).collect::<Vec<String>>().join(", ");
        let values = columns.iter().map(|v| { format!(":{}", v) }).collect::<Vec<String>>().join(", ");
        format!("insert into {} ({}) values ({})", table, column_names, values)
    }

    fn null_or_not(nullable: bool) -> String {
        if nullable {
            String::from("null")
        } else {
            String::from("not null")
        }
    }
}

enumeration!(Order; {Asc: "asc"}, {Desc: "desc"});
