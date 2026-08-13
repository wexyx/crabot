use crabot_domain::{CapabilityRef, CapabilitySource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub entrypoint: PluginEntrypoint,
    pub capabilities: Vec<PluginCapability>,
}

impl PluginManifest {
    pub fn capability_refs(&self) -> Vec<CapabilityRef> {
        self.capabilities
            .iter()
            .map(|capability| CapabilityRef::new(capability.source, capability.name.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginEntrypoint {
    pub command: String,
    pub args: Vec<String>,
    pub transport: PluginTransport,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginTransport {
    JsonRpcStdio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginCapability {
    pub name: String,
    pub description: String,
    pub source: CapabilitySource,
    pub permission: PermissionLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionLevel {
    ReadOnly,
    WorkspaceWrite,
    Network,
    ExternalSideEffect,
}

impl PermissionLevel {
    pub fn requires_approval(self) -> bool {
        matches!(
            self,
            Self::WorkspaceWrite | Self::Network | Self::ExternalSideEffect
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalPolicy {
    pub default: ApprovalDecision,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Ask,
    Deny,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_side_effect_permissions() {
        assert!(!PermissionLevel::ReadOnly.requires_approval());
        assert!(PermissionLevel::WorkspaceWrite.requires_approval());
        assert!(PermissionLevel::ExternalSideEffect.requires_approval());
    }
}
