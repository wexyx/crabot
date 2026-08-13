use anyhow::Error;
use common::tool::ToolSchema;
pub mod hello_world;

pub async fn call(name: &str, params: &str) -> Result<String, Error> {
    let mut params = params;
    if params.len() == 0 {
        params = "{}";
    }
    log::info!("tool calling name: {}, params: {}", name, params);
    let factory = common::registry::factory::<String, Box<dyn common::tool::Tool>>()?;
    let tool = factory
        .create(name, params.to_string())
        .ok_or(anyhow::Error::msg("tool not found"))?;
    tool.run().await
}

pub async fn schema(name: &str) -> Result<ToolSchema, Error> {
    let factory = common::registry::factory::<String, Box<dyn common::tool::Tool>>()?;
    let tool = factory
        .create(name, "{}".to_string())
        .ok_or(anyhow::Error::msg("tool not found"))?;
    Ok(tool.schema().await)
}

pub async fn list() -> Result<Vec<ToolSchema>, Error> {
    let factory = common::registry::factory::<String, Box<dyn common::tool::Tool>>()?;
    let constructors = factory.constructors();
    let mut tools = vec![];
    for (_, c) in constructors {
        let schema = c.create("{}".to_string()).schema().await;
        tools.push(schema);
    }

    Ok(tools)
}
