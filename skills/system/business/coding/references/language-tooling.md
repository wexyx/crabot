# Language tooling selection

Use only the relevant row, then consult the installed version's documentation and repository scripts. These are candidates, not mandatory new dependencies or universal commands.

| Language | Structural analysis / transformation | Tests and coverage |
| --- | --- | --- |
| Rust | rust-analyzer for symbols/types; `syn` for Rust syntax; Clippy for semantic linting; rustfmt for formatting. Preserve proc-macro and cfg behavior. | Existing `cargo test`; cargo-llvm-cov when compatible LLVM tooling is available. Report line and branch availability accurately. |
| TypeScript / JavaScript / Vue | Existing TypeScript compiler API or ts-morph; Babel parser/traverse for supported dialects; `@vue/compiler-sfc` to split Vue SFC blocks before parsing scripts/templates. Preserve TS types, JSX and source maps. | Repo's Vitest/Jest/Node tests and configured V8/Istanbul coverage. Use Vue component tests where rendering or interaction changes. |
| Python | Standard-library `ast` for inspection; LibCST for edits that must preserve comments/format. `ast.unparse` is not a lossless codemod. | pytest/unittest with coverage.py (branch measurement when configured). Use a local venv for missing tools. |
| Go | Standard `go/parser`, `go/ast`, `go/types`; gofmt for edits. Respect build tags and module versions. | `go test` and Go coverage profiles. Do not label statement coverage as branch coverage. |
| Java / Kotlin | Existing compiler/IDE language service; JavaParser or Spoon for Java; Kotlin PSI/compiler tooling for Kotlin. Resolve classpath/toolchain before semantic edits. | Existing Maven/Gradle test task and JaCoCo/Kover configuration. |
| C / C++ | Clang AST/libTooling with the real compilation database, defines and include paths; clang-format. | Existing CTest/test framework and llvm-cov/gcov pipeline. |
| C# | Roslyn syntax/semantic model within the repository's .NET SDK version. | Existing dotnet test and coverage collector. |
| Other | Prefer compiler-native parsing; Tree-sitter is a syntax-only fallback when a grammar exists. | Use established repository tools; report unsupported metrics instead of fabricating them. |

Before installing, check binaries and manifests without running arbitrary repository scripts. Reuse locked dependencies; resolve new tooling from official language/package sources. Keep temporary tools outside tracked source and production dependency graphs. Installing AST tooling is not permission to execute untrusted application code or package lifecycle scripts without the normal execution approval.

Primary documentation entry points:
- Python AST: https://docs.python.org/3/library/ast.html
- TypeScript compiler API: https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API
- Rust syn: https://docs.rs/syn/
- Go AST: https://pkg.go.dev/go/ast
- Clang LibTooling: https://clang.llvm.org/docs/LibTooling.html
