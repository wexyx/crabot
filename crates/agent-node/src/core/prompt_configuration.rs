use super::Core;
use agent_runtime::prompts::{PromptStore, definitions};
use serde_json::{Value, json};

fn snapshot(store: &PromptStore) -> Result<Value, String> {
    let entries = definitions()
        .iter()
        .map(|entry| {
            Ok(
                json!({"id":entry.id,"title":entry.title,"description":entry.description,
            "path":store.path(entry.id)?,"content":store.read(entry.id)?,"default":entry.default,"overridden":store.overridden(entry.id)?}),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({"directory":store.directory(),"prompts":entries}))
}

impl Core {
    pub(crate) async fn system_prompts(&self) -> Result<Value, String> {
        tokio::task::spawn_blocking(|| {
            let store = PromptStore::instance();
            store.validate_all()?;
            snapshot(&store)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    pub(crate) async fn save_system_prompt(&self, input: Value) -> Result<Value, String> {
        tokio::task::spawn_blocking(move || {
            let store = PromptStore::instance();
            let id = input["id"].as_str().ok_or("id required")?;
            let expected = input["expected"].as_str().ok_or("expected required")?;
            if input["reset"] == true {
                store.reset(id, expected)?;
            } else {
                let content = input["content"].as_str().ok_or("content required")?;
                store.save(id, content, expected)?;
            }
            snapshot(&store)
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
