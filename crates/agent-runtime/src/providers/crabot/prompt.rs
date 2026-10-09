pub(crate) fn default_system_prompt() -> &'static str {
    include_str!("../../../../../conf/agent.md").trim_end()
}
