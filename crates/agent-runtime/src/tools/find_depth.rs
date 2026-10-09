use serde_json::{Value, json};

/// How much a single `find` is allowed to return.
///
/// The model picks a depth per call and may go shallower than the ceiling, but the
/// operator's setting is the maximum. Without a knob the only choices were "one
/// result" or "the whole toolset", and the whole toolset is what discovery exists
/// to avoid; depth makes the trade explicit and tunable instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindDepth {
    Brief,
    Normal,
    Deep,
    Exhaustive,
}

/// Per-depth result ceilings. Tools and skills are in-memory registries so their
/// counts are small; history is a disk scan and is bounded more tightly.
impl FindDepth {
    pub const ALL: [FindDepth; 4] = [
        FindDepth::Brief,
        FindDepth::Normal,
        FindDepth::Deep,
        FindDepth::Exhaustive,
    ];
    pub fn as_str(&self) -> &'static str {
        match self {
            FindDepth::Brief => "brief",
            FindDepth::Normal => "normal",
            FindDepth::Deep => "deep",
            FindDepth::Exhaustive => "exhaustive",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            FindDepth::Brief => "简要",
            FindDepth::Normal => "标准",
            FindDepth::Deep => "深入",
            FindDepth::Exhaustive => "详尽",
        }
    }
    /// Hard ceiling for catalog hits. A caller may request fewer.
    pub fn tool_limit(&self) -> usize {
        match self {
            FindDepth::Brief => 5,
            FindDepth::Normal => 10,
            FindDepth::Deep => 25,
            FindDepth::Exhaustive => 50,
        }
    }
    pub fn skill_limit(&self) -> usize {
        self.tool_limit()
    }
    pub fn history_limit(&self) -> usize {
        match self {
            FindDepth::Brief => 5,
            FindDepth::Normal => 15,
            FindDepth::Deep => 50,
            FindDepth::Exhaustive => 100,
        }
    }
    /// Whether a hit carries its full JSON schema. Even at depth, only the tools
    /// that actually matched are described, which is what keeps this bounded.
    pub fn includes_schema(&self) -> bool {
        matches!(self, FindDepth::Deep | FindDepth::Exhaustive)
    }
    // No scan budget: history is served by an index, not a linear log walk, so there
    // is no "records read before giving up" for depth to scale. Depth scales the
    // returned hits through `history_limit` instead.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "normal" | "standard" | "2" => Ok(FindDepth::Normal),
            "brief" | "quick" | "shallow" | "1" => Ok(FindDepth::Brief),
            "deep" | "3" => Ok(FindDepth::Deep),
            "exhaustive" | "max" | "maximum" | "full" | "4" => Ok(FindDepth::Exhaustive),
            other => Err(format!(
                "find depth must be brief, normal, deep or exhaustive, not {other:?}"
            )),
        }
    }
    /// The smaller of what was asked for and what the operator allows.
    ///
    /// A model asking for `max` must not be able to exceed the configured ceiling,
    /// otherwise the setting would only be a suggestion. Deliberately named `capped`
    /// rather than `clamp` so it cannot be confused with `Ord::clamp`, which takes a
    /// range of two depths and would silently accept a reversed pair.
    pub fn capped_by(self, ceiling: FindDepth) -> Self {
        if self <= ceiling { self } else { ceiling }
    }
    /// Resolve the effective depth for a call: the model's choice, the operator's
    /// ceiling, and any explicit `limit` that must still fit inside it.
    pub fn resolve(
        requested: Option<&str>,
        limit: Option<u64>,
        ceiling: FindDepth,
    ) -> (Self, usize) {
        // An unparsable depth falls back to the ceiling instead of failing the call:
        // the model gets results either way, and a typo should not stall a task.
        let asked = requested
            .and_then(|value| FindDepth::parse(value).ok())
            .unwrap_or(ceiling)
            .capped_by(ceiling);
        let cap = asked.tool_limit() as u64;
        // Zero is not a meaningful request, so it takes the whole budget rather than
        // silently returning nothing.
        let capped = match limit {
            Some(0) | None => cap,
            Some(requested) => requested.clamp(1, cap),
        };
        (asked, capped as usize)
    }

    /// The history-specific half of `resolve`, whose ceiling differs from the
    /// catalog's because it is bounded by a disk scan rather than by memory.
    pub fn resolve_history(
        requested: Option<&str>,
        limit: Option<u64>,
        ceiling: FindDepth,
    ) -> (Self, usize) {
        let asked = requested
            .and_then(|value| FindDepth::parse(value).ok())
            .unwrap_or(ceiling)
            .capped_by(ceiling);
        let cap = asked.history_limit() as u64;
        let capped = match limit {
            Some(0) | None => cap,
            Some(requested) => requested.clamp(1, cap),
        };
        (asked, capped as usize)
    }
    /// Host-facing description of the current ceiling, for `/status` and the web UI.
    pub fn describe(&self) -> Value {
        json!({
            "depth":self.as_str(),
            "label":self.label(),
            "tools":self.tool_limit(),
            "skills":self.skill_limit(),
            "history":self.history_limit(),
            "schema":self.includes_schema(),
        })
    }
}

/// The operator-configured ceiling, read from the environment the runtime was
/// built with. Defaults to `normal` so an unconfigured node behaves conservatively.
pub fn ceiling() -> FindDepth {
    FindDepth::parse(&std::env::var("AGENT_FIND_DEPTH").unwrap_or_default())
        .unwrap_or(FindDepth::Normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_orders_and_parses_every_spelling() {
        assert!(FindDepth::Brief < FindDepth::Normal);
        assert!(FindDepth::Deep < FindDepth::Exhaustive);
        for depth in FindDepth::ALL {
            assert_eq!(FindDepth::parse(depth.as_str()).unwrap(), depth);
        }
        assert_eq!(FindDepth::parse("").unwrap(), FindDepth::Normal);
        assert_eq!(FindDepth::parse("MAX").unwrap(), FindDepth::Exhaustive);
        assert_eq!(FindDepth::parse("1").unwrap(), FindDepth::Brief);
        assert!(FindDepth::parse("infinite").is_err());
    }

    /// The whole point of the setting: a model asking for max is bounded by it.
    #[test]
    fn a_request_deeper_than_the_ceiling_is_clamped() {
        assert_eq!(
            FindDepth::Exhaustive.capped_by(FindDepth::Normal),
            FindDepth::Normal
        );
        assert_eq!(
            FindDepth::Brief.capped_by(FindDepth::Deep),
            FindDepth::Brief
        );
    }

    #[test]
    fn resolve_clamps_both_depth_and_limit_to_the_ceiling() {
        let (depth, limit) = FindDepth::resolve(Some("max"), None, FindDepth::Normal);
        assert_eq!(depth, FindDepth::Normal);
        assert_eq!(limit, FindDepth::Normal.tool_limit());

        let (depth, limit) = FindDepth::resolve(Some("brief"), Some(999), FindDepth::Deep);
        assert_eq!(depth, FindDepth::Brief);
        assert_eq!(limit, FindDepth::Brief.tool_limit());
    }

    #[test]
    fn an_unparsable_depth_falls_back_to_the_ceiling_rather_than_failing() {
        let (depth, _) = FindDepth::resolve(Some("galore"), None, FindDepth::Deep);
        assert_eq!(depth, FindDepth::Deep);
    }

    #[test]
    fn an_absent_limit_takes_the_whole_budget_and_a_present_one_is_honoured() {
        let (_, limit) = FindDepth::resolve(Some("normal"), None, FindDepth::Deep);
        assert_eq!(limit, FindDepth::Normal.tool_limit());
        let (_, limit) = FindDepth::resolve(Some("deep"), Some(3), FindDepth::Deep);
        assert_eq!(limit, 3);
        // Zero is not a meaningful request; treat it as "whatever the depth allows".
        let (_, limit) = FindDepth::resolve(Some("deep"), Some(0), FindDepth::Deep);
        assert_eq!(limit, FindDepth::Deep.tool_limit());
    }

    #[test]
    fn every_depth_grows_monotonically() {
        let mut previous = 0;
        for depth in FindDepth::ALL {
            assert!(depth.tool_limit() > previous);
            previous = depth.tool_limit();
            assert!(depth.history_limit() >= depth.tool_limit());
        }
    }
}
