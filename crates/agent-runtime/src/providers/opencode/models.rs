use crate::environment::AgentEnvironment;
use serde::Deserialize;
use serde::Serialize;
use std::{path::Path, process::Stdio, time::Duration};

/// Discovering the catalog may have to start or reach the background service.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// One model the local OpenCode install is willing to route to.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Model {
    /// `provider/model`, exactly what `opencode run --model` expects.
    pub id: String,
    pub name: String,
    /// Every published tier costs zero, so no plan is required to call it.
    pub free: bool,
    /// USD per million tokens at the base tier. `None` when the install only
    /// reported identifiers, which is why `priced` exists alongside it.
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub context: Option<u64>,
    /// The model advertises tool calling, which Crabot sessions require.
    pub tools: bool,
    pub status: String,
    /// `false` means costs are unknown. Never infer "free" from a missing price.
    pub priced: bool,
}
/// How complete the answer was, so a caller can say so instead of guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// `/api/model` answered, so prices, limits and capabilities are real.
    Catalog,
    /// Only `opencode models` answered, so nothing here may claim a price.
    Identifiers,
}
impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Identifiers => "identifiers",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub models: Vec<Model>,
    pub source: Source,
}

/// Ask the configured OpenCode install which models it can route to.
///
/// The catalog belongs to the host profile, not to an Agent workspace: OpenCode
/// keeps credentials in its own data directory, and a private (`--standalone`)
/// install reports no models at all. Discovery therefore runs against the host
/// environment and only ever reads; it never executes an Agent session.
pub async fn discover(config: &super::config::OpenCodeConfig) -> Result<Catalog, String> {
    // The launcher's extra arguments are session options; appending a read-only
    // subcommand after them would be rejected, so only the executable is reused.
    let binary = config.launch()?.binary().to_path_buf();
    let environment = config.environment.clone();
    match catalog(&binary, &environment).await {
        Ok(models) => Ok(Catalog {
            models,
            source: Source::Catalog,
        }),
        Err(error) => identifiers(&binary, &environment)
            .await
            .map(|models| Catalog {
                models,
                source: Source::Identifiers,
            })
            .map_err(|fallback| format!("{error}；回退到 opencode models 同样失败：{fallback}")),
    }
}
async fn catalog(binary: &Path, environment: &AgentEnvironment) -> Result<Vec<Model>, String> {
    let output = run(binary, &["api", "get", "/api/model"], environment).await?;
    let parsed: Response =
        serde_json::from_str(&output).map_err(|_| "无法解析 OpenCode 模型目录".to_string())?;
    Ok(rank(parsed.data.into_iter().filter_map(convert).collect()))
}
async fn identifiers(binary: &Path, environment: &AgentEnvironment) -> Result<Vec<Model>, String> {
    let output = run(binary, &["models"], environment).await?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| line.contains('/'))
        .map(|id| Model {
            name: id
                .split_once('/')
                .map(|(_, model)| model)
                .unwrap_or(id)
                .into(),
            id: id.into(),
            free: false,
            input: None,
            output: None,
            context: None,
            tools: true,
            status: String::new(),
            priced: false,
        })
        .collect())
}
async fn run(
    binary: &Path,
    arguments: &[&str],
    environment: &AgentEnvironment,
) -> Result<String, String> {
    let mut command = tokio::process::Command::new(binary);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    environment.overlay(&mut command);
    let output = tokio::time::timeout(DISCOVERY_TIMEOUT, command.output())
        .await
        .map_err(|_| "读取 OpenCode 模型超时".to_string())?
        .map_err(|e| format!("无法执行 {}：{e}", binary.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} 退出码 {}：{}",
            binary.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "OpenCode 模型列表不是 UTF-8".into())
}

/// Free models first, then cheapest, so a picker never hides the usable defaults.
fn rank(models: Vec<Model>) -> Vec<Model> {
    let mut models = models;
    models.sort_by(|a, b| {
        b.free
            .cmp(&a.free)
            .then(
                a.input
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.input.unwrap_or(f64::MAX)),
            )
            .then(a.id.cmp(&b.id))
    });
    models
}
fn convert(entry: Entry) -> Option<Model> {
    if entry.enabled == Some(false) {
        return None;
    }
    let model = entry.id.trim();
    let provider = entry.provider_id.trim();
    let id = if provider.is_empty() {
        model.to_string()
    } else {
        format!("{provider}/{model}")
    };
    let priced = !entry.cost.is_empty();
    let free = priced
        && entry.cost.iter().all(|tier| {
            tier.input == 0.0
                && tier.output == 0.0
                && tier
                    .cache
                    .as_ref()
                    .is_none_or(|cache| cache.read == 0.0 && cache.write == 0.0)
        });
    Some(Model {
        name: if entry.name.trim().is_empty() {
            model.into()
        } else {
            entry.name
        },
        id,
        free,
        input: entry.cost.first().map(|tier| tier.input),
        output: entry.cost.first().map(|tier| tier.output),
        context: entry.limit.map(|limit| limit.context),
        tools: entry
            .capabilities
            .map(|capabilities| capabilities.tools)
            .unwrap_or(true),
        status: entry.status,
        priced,
    })
}
#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    data: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    #[serde(default)]
    id: String,
    #[serde(rename = "providerID", default)]
    provider_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    cost: Vec<Tier>,
    #[serde(default)]
    status: String,
    enabled: Option<bool>,
    limit: Option<Limit>,
    capabilities: Option<Capabilities>,
}
#[derive(Deserialize)]
struct Tier {
    #[serde(default)]
    input: f64,
    #[serde(default)]
    output: f64,
    cache: Option<Cache>,
}
#[derive(Deserialize)]
struct Cache {
    #[serde(default)]
    read: f64,
    #[serde(default)]
    write: f64,
}
#[derive(Deserialize)]
struct Limit {
    #[serde(default)]
    context: u64,
}
#[derive(Deserialize)]
struct Capabilities {
    #[serde(default)]
    tools: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(raw: &str) -> Entry {
        serde_json::from_str(raw).unwrap()
    }
    #[test]
    fn zero_cost_is_free_and_missing_cost_is_unknown() {
        let free = convert(entry(
            r#"{"id":"space-bunny-free","providerID":"opencode","name":"Space Bunny",
                "cost":[{"input":0,"output":0,"cache":{"read":0,"write":0}}],
                "limit":{"context":1048576},"capabilities":{"tools":true},"status":"active"}"#,
        ))
        .unwrap();
        assert_eq!(free.id, "opencode/space-bunny-free");
        assert!(free.free && free.priced && free.tools);
        assert_eq!(free.context, Some(1048576));
        let paid = convert(entry(
            r#"{"id":"gpt-6.1-sol","providerID":"opencode",
                "cost":[{"input":2,"output":10,"cache":{"read":0.1,"write":2.5}},
                        {"tier":{"type":"context","size":272000},"input":4,"output":15}],
                "limit":{"context":1050000},"capabilities":{"tools":false},"status":"active"}"#,
        ))
        .unwrap();
        assert!(!paid.free && paid.priced);
        assert_eq!((paid.input, paid.output), (Some(2.0), Some(10.0)));
        assert!(!paid.tools);
        let unknown = convert(entry(r#"{"id":"local","providerID":"ollama"}"#)).unwrap();
        assert!(!unknown.free && !unknown.priced);
        assert_eq!(
            (unknown.input, unknown.output, unknown.context),
            (None, None, None)
        );
        // A disabled model must not be offered at all.
        assert!(
            convert(entry(
                r#"{"id":"off","providerID":"opencode","enabled":false}"#
            ))
            .is_none()
        );
    }
    #[test]
    fn catalog_ranks_free_before_paid_and_priced_before_unknown() {
        let paid = convert(entry(
            r#"{"id":"paid","providerID":"opencode","cost":[{"input":2,"output":10}]}"#,
        ))
        .unwrap();
        let free = convert(entry(
            r#"{"id":"free","providerID":"opencode","cost":[{"input":0,"output":0}]}"#,
        ))
        .unwrap();
        let unknown = convert(entry(r#"{"id":"unknown","providerID":"opencode"}"#)).unwrap();
        let ranked: Vec<String> = rank(vec![unknown, paid, free])
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(
            ranked,
            vec!["opencode/free", "opencode/paid", "opencode/unknown"]
        );
        assert_eq!(Source::Catalog.as_str(), "catalog");
        assert_eq!(
            serde_json::to_string(&Source::Identifiers).unwrap(),
            "\"identifiers\""
        );
    }
}
