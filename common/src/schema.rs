use anyhow::Error;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub trait SchemaParams {
    fn into_schema() -> Schema;
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Schema {
    pub data: serde_json::Value,
}

impl Schema {
    pub fn new(data: serde_json::Value) -> Self {
        Self { data }
    }
}

impl Display for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let params: Result<openai_api_rs::v1::types::FunctionParameters, Error> =
            self.clone().into();
        let data = params
            .map(|v| serde_json::to_string_pretty(&v).unwrap())
            .unwrap_or_else(|e| e.to_string());
        f.write_str(data.as_str())
    }
}

impl From<Schema> for Result<openai_api_rs::v1::types::FunctionParameters, Error> {
    fn from(value: Schema) -> Self {
        let data = serde_json::from_value(value.data);
        data.map_err(Into::into)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Empty {}

impl Empty {
    pub fn new() -> Self {
        Self {}
    }
}

impl SchemaParams for Empty {
    fn into_schema() -> Schema {
        Schema {
            data: serde_json::Value::Null,
        }
    }
}
