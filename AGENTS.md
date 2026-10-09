# Rust architecture conventions

- `mod.rs` is only for module declarations and symbol re-exports. Put interfaces, types, constructors, implementations and tests in dedicated files. Keep `lib.rs` as the public export facade as well.
- Mock, Crabot (our own Harness), Codex and Claude are peer providers implementing the same Provider contract. Keep concrete configuration, execution state and vendor protocol parsing inside their respective provider modules.
- Encapsulate runtime state in structs with private fields and associated constructors. The factory selects and constructs providers; the shared runtime owns lifecycle/event normalization. Do not add vendor-specific execution branches to callers.
