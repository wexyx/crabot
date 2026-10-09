---
name: coding
description: Implement and refactor code using repository conventions, clean responsibilities, language-aware analysis and meaningful tests targeting at least 90 percent coverage.
---

# Coding

Apply to authorized code changes and test work, not as permission to implement fixes when the user only requests an explanation or review.

## Understand the repository

Read applicable `AGENTS.md`, contributor instructions, manifests, formatter/linter configs, CI and representative neighboring implementations/tests. Inspect Git status and recent relevant history to preserve established naming, module boundaries, error handling and architecture. User and repository rules outrank generic style preferences. Preserve unrelated changes; do not auto-commit, push, install global tools, or rewrite unrelated code.

## Implement with clear responsibilities

Choose the smallest coherent design: one responsibility per module/function, explicit dependencies and state, meaningful names, idiomatic interfaces and no speculative abstraction. Reuse existing factories, contracts and utilities. Separate business logic, I/O and presentation. Validate boundaries and model success, errors, cancellation, cleanup, retries and concurrent execution explicitly where applicable. Do not silently swallow failures or weaken authorization. Cover partial completion and repeated requests when there are side effects.

## Language-aware edits

Read [Language tooling](references/language-tooling.md) for the language being changed. Prefer the project's installed compiler, language service or AST parser for symbol-aware investigation and structural changes. AST parsing does not replace type checking, tests or behavior verification. Use it when it improves confidence; do not add a parser dependency for a trivial textual edit. Inspect generated diffs and run the native formatter. Avoid regex rewrites across arbitrary syntax or changing comments/strings as if they were code.

If a suitable AST tool is missing, inspect the repo's toolchain/package manager first and propose or attempt a scoped installation through `shell`, subject to the normal approval policy. Prefer an instance-local tool environment or temporary workspace over modifying production dependencies. Verify the official package/source and compatible version before installing; no arbitrary install scripts from search results, `sudo`, global installs or manifest/lockfile changes without authorization. If installation is denied, unavailable or incompatible, use compiler diagnostics/manual inspection and clearly report the fallback; never claim AST analysis ran.

## Verify rather than assume

- Add regression tests for the reported bug before or alongside the fix. Cover normal behavior, boundary/invalid inputs, failure/cleanup and meaningful branches; use isolated fixtures and avoid real accounts, network side effects or credentials.
- Target **at least 90% unit-test line coverage for new/changed modules**, and at least 90% branch coverage where the language/tool supports measuring it. Honor stricter repo thresholds. If the existing project already measures whole-project coverage, run and report that metric too; do not imply a changed-module score is whole-project coverage.
- Obtain coverage from an actual report; record the command, metric, scope, numerator/denominator when available and uncovered important paths. Do not exclude difficult files, add meaningless tests, mock away the behavior under test or weaken assertions just to reach 90%. Cover critical safety paths even when the numerical target already passes.
- Run targeted tests first, then required broader tests, formatter, linter, compiler/type checker and integration checks proportionate to the change. Re-run affected checks after fixes.
- If tooling, environment or the pre-existing baseline prevents 90%, report the measured gap or that coverage is unmeasured, explain why, and identify remaining work. Do not invent success or expand into unrelated rewrites to raise the score.

Finish with what changed, verification evidence and remaining limits. Keep the explanation short; let the code and tests carry the detail.
