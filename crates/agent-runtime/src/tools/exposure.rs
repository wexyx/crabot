use super::ToolDefinition;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// The entry points a model is always allowed to see.
///
/// Built-ins registered by the factory are also visible immediately. External
/// definitions remain discoverable so an unbounded user catalog is not inlined.
pub const DISCOVERY_TOOLS: [&str; 1] = ["find"];

/// Whether a name is always advertised, independent of what the model has unlocked.
pub fn is_discovery(name: &str) -> bool {
    DISCOVERY_TOOLS.contains(&name)
}

/// A shared mirror of the registered tool definitions.
///
/// The registry owns the tools, so a tool that describes the registry cannot hold a
/// reference to it without a cycle. This carries owned definition values instead:
/// the factory publishes them once, after every tool is registered and disabled
/// entries have been removed.
#[derive(Clone, Default)]
pub struct ToolIndex {
    entries: Arc<RwLock<BTreeMap<String, ToolDefinition>>>,
}
impl ToolIndex {
    pub fn new() -> Self {
        Self::default()
    }
    /// Replace the mirror with the registry's current snapshot.
    pub(crate) fn publish(&self, definitions: Vec<ToolDefinition>) {
        let mut entries = self.write();
        entries.clear();
        for definition in definitions {
            entries.insert(definition.name().into(), definition);
        }
    }
    pub fn entries(&self) -> Vec<ToolDefinition> {
        self.read().values().cloned().collect()
    }
    pub fn get(&self, name: &str) -> Option<ToolDefinition> {
        self.read().get(name).cloned()
    }
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }
    // A poisoned lock means a previous holder panicked mid-write. The mirror is a
    // rebuildable cache, so recovering the guard beats propagating the panic into
    // every later tool call.
    fn read(&self) -> std::sync::RwLockReadGuard<'_, BTreeMap<String, ToolDefinition>> {
        self.entries.read().unwrap_or_else(|e| e.into_inner())
    }
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, BTreeMap<String, ToolDefinition>> {
        self.entries.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn definition(name: &str) -> ToolDefinition {
        ToolDefinition::new(name, "d", json!({"type": "object"}))
    }
    #[test]
    fn publish_replaces_the_previous_snapshot() {
        let index = ToolIndex::new();
        index.publish(vec![definition("shell")]);
        assert!(!index.is_empty());
        assert!(index.get("shell").is_some());
        index.publish(vec![definition("compact")]);
        assert!(index.get("shell").is_none());
        assert!(index.get("compact").is_some());
    }
    #[test]
    fn find_is_the_discovery_entry_point_independent_of_builtin_registration() {
        assert!(is_discovery("find"));
        assert!(!is_discovery("compact"));
        // The pre-merge names must not linger: a stale definition would advertise an
        // entry point that no longer exists.
        for retired in ["find_tools", "find_skills", "find_history", "find_doc"] {
            assert!(!is_discovery(retired), "{retired}");
        }
        assert!(!is_discovery("shell"));
    }
}
