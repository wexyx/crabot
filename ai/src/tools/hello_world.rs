use anyhow::Error;
use common::schema::SchemaParams;
use derive::{Scheme, tool_function};
use serde::{Deserialize, Serialize};

#[derive(Scheme, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HelloWorldReq {
    #[field(desc = "用户名")]
    pub name: String,
}

#[derive(Scheme, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HelloWorldResp {
    #[field(desc = "问候信息")]
    pub msg: String,
}

#[tool_function(desc = "打印hello_world")]
pub async fn print_hello_world(_req: HelloWorldReq) -> Result<HelloWorldResp, Error> {
    Ok(HelloWorldResp {
        msg: "how are u".to_string(),
    })
}
