use super::{Context, Resource};
use crate::storage::Store;
use agent_runtime::{
    skills::{SkillCatalog, SkillDefinition},
    tools::ToolPolicy,
};
use serde_json::{Value, json};
use std::collections::HashSet;
#[derive(Clone)]
pub(crate) struct Library {
    store: Store,
}
impl Library {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
    // Legacy rows are exposed as library entries without copying their content or widening access.
    pub async fn resources(&self, scope: &str, kind: &str) -> Result<Vec<Resource>, String> {
        if !matches!(scope, "business" | "management") || !matches!(kind, "skill" | "tool") {
            return Err("invalid capability scope or kind".into());
        }
        let mut result = vec![];
        for row in self.store.list("capability_library").await {
            if row["scope"] == scope && row["kind"] == kind && row["deleted"] != true {
                let row = self.store.skill_row("capability_library", &row)?;
                result.push(serde_json::from_value::<Resource>(row).map_err(|e| e.to_string())?);
            }
        }
        if kind == "skill" {
            let collection = if scope == "business" {
                "skills"
            } else {
                "management_skills"
            };
            for row in self.store.list(collection).await {
                if row["deleted"] == true {
                    continue;
                }
                let row = self.store.skill_row(collection, &row)?;
                let definition = row[if scope == "business" {
                    "skill"
                } else {
                    "definition"
                }]
                .clone();
                let project = row["project_id"].as_str().ok_or("invalid legacy project")?;
                result.push(Resource {
                    id: format!(
                        "legacy:{scope}:skill:{project}:{}",
                        definition["id"].as_str().unwrap_or("")
                    ),
                    kind: kind.into(),
                    scope: scope.into(),
                    definition,
                    version: row[if scope == "business" {
                        "revision"
                    } else {
                        "version"
                    }]
                    .as_u64()
                    .unwrap_or(0),
                    readonly: false,
                    origin: Some(json!({"project":project})),
                });
            }
            let system = super::system_skills::resources(scope).await?;
            if scope == "management" && !system.iter().any(|r| r.name() == "management-guide") {
                result.push(super::system_skills::fallback_management_guide());
            }
            result.extend(system);
        } else {
            for row in self.store.list("tool_policies").await {
                if row["scope"] != scope {
                    continue;
                }
                for definition in row["policy"]["external"].as_array().into_iter().flatten() {
                    let project = row["project_id"].as_str().ok_or("invalid legacy project")?;
                    let agent = row["agent"].as_str().ok_or("invalid legacy agent")?;
                    result.push(Resource {
                        id: format!(
                            "legacy:{scope}:tool:{project}:{agent}:{}",
                            definition["name"].as_str().unwrap_or("")
                        ),
                        kind: kind.into(),
                        scope: scope.into(),
                        definition: definition.clone(),
                        version: row["version"].as_u64().unwrap_or(0),
                        readonly: false,
                        origin: Some(json!({"project":project,"agent":agent})),
                    });
                }
            }
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(result)
    }
    pub async fn skills(&self, scope: &str, context: &Context) -> Result<SkillCatalog, String> {
        let mut definitions = vec![];
        let mut builtins = vec![];
        for item in self.resources(scope, "skill").await? {
            if context.resolve(&self.store, &item, None).await?["enabled"] == true {
                if item.readonly {
                    builtins.push(item.name().to_owned());
                }
                let mut definition = item.definition;
                definition["enabled"] = json!(true);
                definitions.push(serde_json::from_value(definition).map_err(|e| e.to_string())?);
            }
        }
        Ok(SkillCatalog::new(definitions)?.with_builtins(builtins))
    }
    /// Execution snapshots keep their frozen enablement. Recognize a built-in only
    /// when its content matches the local installed definition, not by name alone.
    pub(crate) async fn classify_skills(
        &self,
        scope: &str,
        catalog: SkillCatalog,
    ) -> Result<SkillCatalog, String> {
        let installed = self.resources(scope, "skill").await?;
        let builtins = catalog
            .definitions()
            .into_iter()
            .filter(|skill| {
                installed.iter().any(|r| {
                    r.readonly
                        && r.name() == skill.id()
                        && r.definition["files"] == json!(skill.files())
                        && r.definition["description"] == skill.description()
                })
            })
            .map(|s| s.id().to_owned())
            .collect::<Vec<_>>();
        Ok(catalog.with_builtins(builtins))
    }
    pub async fn tool_policy(&self, scope: &str, context: &Context) -> Result<ToolPolicy, String> {
        let legacy = self
            .store
            .get(
                "tool_policies",
                &format!("{}:{scope}:{}", context.namespace, context.agent),
            )
            .await;
        let mut disabled: HashSet<String> = legacy
            .as_ref()
            .and_then(|r| r["policy"]["disabled"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_owned))
            .collect();
        // Built-in bindings can exist even when a context-dependent factory omits that tool.
        let mut names = disabled.clone();
        for row in self.store.list("capability_bindings").await {
            if let Some(name) = row["resource"]
                .as_str()
                .and_then(|id| id.strip_prefix(&format!("builtin:{scope}:tool:")))
            {
                names.insert(name.into());
            }
        }
        for name in names {
            let resource = Resource {
                id: format!("builtin:{scope}:tool:{name}"),
                kind: "tool".into(),
                scope: scope.into(),
                definition: json!({"name":name}),
                version: 0,
                readonly: true,
                origin: None,
            };
            if context
                .resolve(&self.store, &resource, Some(!disabled.contains(&name)))
                .await?["enabled"]
                == true
            {
                disabled.remove(&name);
            } else {
                disabled.insert(name);
            }
        }
        let mut external = vec![];
        let mut active = HashSet::new();
        for item in self.resources(scope, "tool").await? {
            if context.resolve(&self.store, &item, None).await?["enabled"] == true {
                if !active.insert(item.name().to_owned()) {
                    return Err(format!(
                        "conflicting enabled tool definitions: {}; disable one binding",
                        item.name()
                    ));
                }
                let mut definition = item.definition;
                definition["enabled"] = json!(true);
                external.push(definition);
            }
        }
        let policy: ToolPolicy =
            serde_json::from_value(json!({"disabled":disabled,"external":external}))
                .map_err(|e| e.to_string())?;
        policy.validate()?;
        Ok(policy)
    }
    pub async fn save(&self, scope: &str, kind: &str, input: Value) -> Result<Value, String> {
        let id = input["id"].as_str().unwrap_or("").to_owned();
        let version = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let existing = self
            .resources(scope, kind)
            .await?
            .into_iter()
            .find(|r| r.id == id);
        if !id.is_empty() && existing.is_none() {
            return Err("unknown capability".into());
        }
        if existing.as_ref().is_some_and(|r| r.readonly) {
            return Err("built-in definitions are read-only".into());
        }
        let deleted = input["deleted"] == true;
        let definition = if deleted {
            existing
                .as_ref()
                .ok_or("definition required")?
                .definition
                .clone()
        } else {
            input["definition"].clone()
        };
        if kind == "skill" {
            let skill: SkillDefinition =
                serde_json::from_value(definition.clone()).map_err(|e| e.to_string())?;
            skill.validate()?;
            if skill.allow_python()
                && !existing
                    .as_ref()
                    .is_some_and(|r| r.definition["allow_python"] == true)
            {
                return Err("cannot grant script execution permissions".into());
            }
            if scope == "management" && (skill.allow_python() || skill.id() == "management-guide") {
                return Err("reserved management Skill or Python permission".into());
            }
            if existing.as_ref().is_some_and(|r| r.name() != skill.id()) {
                return Err("Skill ID cannot change".into());
            }
        } else {
            if scope != "business" {
                return Err("external tools require business scope".into());
            }
            let policy: ToolPolicy = serde_json::from_value(json!({"external":[definition]}))
                .map_err(|e| e.to_string())?;
            policy.validate()?;
            if existing
                .as_ref()
                .is_some_and(|r| r.name() != definition["name"].as_str().unwrap_or(""))
            {
                return Err("tool name cannot change".into());
            }
        }
        if existing.is_none()
            && self.resources(scope, kind).await?.iter().any(|r| {
                r.name()
                    == definition[if kind == "skill" { "id" } else { "name" }]
                        .as_str()
                        .unwrap_or("")
            })
        {
            return Err("name already exists in shared library".into());
        }
        let saved_id = if id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            id
        };
        self.store.transaction(|data|{
            let next=version.checked_add(1).ok_or("version overflow")?;
            if let Some(origin)=existing.as_ref().and_then(|r|r.origin.as_ref()) {
                let project=origin["project"].as_str().ok_or("invalid origin")?;
                if kind=="skill" {
                    let collection=if scope=="business"{"skills"}else{"management_skills"};
                    let key=format!("{project}:{}",existing.as_ref().unwrap().name());
                    let mut row=data.get(collection,&key).cloned().ok_or("missing legacy definition")?;
                    let vf=if scope=="business"{"revision"}else{"version"};
                    if row[vf]!=version {return Err("version_conflict".into())}
                    row[vf]=json!(next);row[if scope=="business"{"skill"}else{"definition"}]=definition.clone();row["deleted"]=json!(deleted);data.set(collection,&key,row);
                }else{
                    let key=format!("{project}:{scope}:{}",origin["agent"].as_str().ok_or("invalid origin")?);
                    let mut row=data.get("tool_policies",&key).cloned().ok_or("missing tool policy")?;
                    if row["version"]!=version {return Err("version_conflict".into())}
                    let items=row["policy"]["external"].as_array_mut().ok_or("invalid tool policy")?;
                    let index=items.iter().position(|v|v["name"]==definition["name"]).ok_or("missing tool")?;
                    if deleted{items.remove(index);}else{items[index]=definition.clone();}
                    row["version"]=json!(next);data.set("tool_policies",&key,row);
                }
            }else{
                let old=data.get("capability_library",&saved_id).and_then(|r|r["version"].as_u64()).unwrap_or(0);
                if old!=version {return Err("version_conflict".into())}
                let name_key=if kind=="skill"{"id"}else{"name"};
                if existing.is_none() && data.list("capability_library").iter().any(|r|r["scope"]==scope&&r["kind"]==kind&&r["deleted"]!=true&&r["definition"][name_key]==definition[name_key]) {return Err("name already exists".into())}
                data.set("capability_library",&saved_id,json!({"id":saved_id,"scope":scope,"kind":kind,"definition":definition,"version":next,"readonly":false,"origin":null,"deleted":deleted}));
            }
            Ok(json!({"id":saved_id,"version":next,"deleted":deleted}))
        }).await
    }
    pub async fn bind(
        &self,
        resource: &str,
        context: &Context,
        input: Value,
    ) -> Result<Value, String> {
        let layer = input["layer"].as_str().ok_or("layer required")?;
        let enabled = &input["enabled"];
        if !(enabled.is_boolean() || enabled.is_null()) {
            return Err("enabled must be true, false or null (inherit)".into());
        }
        let version = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let key = context.key(resource, layer)?;
        self.store.transaction(|data|{
            let old=data.get("capability_bindings",&key).and_then(|r|r["version"].as_u64()).unwrap_or(0);
            if old!=version{return Err("version_conflict".into())}
            let row=json!({"resource":resource,"layer":layer,"enabled":enabled,"version":version.checked_add(1).ok_or("version overflow")?});
            data.set("capability_bindings",&key,row.clone());Ok(row)
        }).await
    }
}
