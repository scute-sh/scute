# ADR-0004: Check Implementation Architecture

**Status:** Accepted
**Date:** 2026-02-27

## Context

Scute needs many check implementations. Some require deep code analysis (cyclomatic
complexity, circular dependencies). Most wrap existing external tools (`cargo audit`,
`jest --coverage`, `npm outdated`) and normalize their output. Users also need
proprietary or domain-specific checks without contributing to the main repository.

Three constraints shape the architecture:

1. **Performance where it matters.** Checks that parse ASTs or traverse dependency
   graphs benefit from native implementation.
2. **Ecosystem leverage.** Most measurements already have good tooling. Wrapping is
   cheaper and more maintainable than reimplementing.
3. **Extensibility without Rust.** Requiring Rust to write a custom check would
   limit adoption. The check result schema (ADR-0001) is language-agnostic — the
   implementation architecture should be too.

## Decision

Three tiers of check implementation. All three produce the same check result
(ADR-0001). Consumers don't know or care which tier generated a result.

### Tier 1: Native

Rust implementation using tree-sitter or direct analysis. For built-in checks where
performance or deep AST access matters.

Examples: cyclomatic complexity, circular dependency detection, layer violation
analysis, naming convention enforcement.

These checks live in the main repository as Rust modules.

### Tier 2: Subprocess Wrapper

Rust glue code that invokes an external tool via `std::process::Command`, parses its
output, and normalizes the result into the check result schema.

Examples:
- Dependency freshness → wraps `npm outdated --json`, `cargo outdated --format json`
- Test coverage → wraps `jest --coverage --json`, `cargo llvm-cov --json`
- Known vulnerabilities → wraps `cargo audit --json`, `npm audit --json`

The wrapper handles:
- Tool invocation and exit code interpretation
- Output parsing (JSON, XML, plaintext — whatever the tool emits)
- Mapping to `observed` + `evidence`
- Applying `thresholds` from the check definition (ADR-0002)

These checks live in the main repository as Rust modules. The Rust code is thin
glue — the external tool does the heavy lifting.

### Tier 3: External Executable

Any executable that conforms to a check protocol. For custom, proprietary, or
domain-specific checks written in any language.

The contract is simple: Scute provides the check definition and target as input, the
executable produces a check result (ADR-0001) as JSON on stdout. The exact invocation
protocol (how input is passed, how the executable is registered in the check
definition) is deferred to implementation.

What matters at the architecture level:

- **Any language.** A Python script, a shell script, a Go binary — anything
  executable.
- **Same output contract.** The executable produces JSON conforming to ADR-0001.
  Scute validates it at the boundary.
- **Separation of check status from execution status.** A check that executes
  successfully but finds violations is not the same as a check that fails to run.
  The protocol must distinguish these.

## Design Decisions

### Subprocess execution, not plugins

Checks invoke external tools and executables via subprocess. No plugin system, no
dynamic library loading, no embedded runtime.

Subprocess is the simplest model that satisfies all requirements:
- **Language-agnostic.** Anything executable is a valid check.
- **Isolated.** A misbehaving check can't corrupt the engine's memory or state.
- **Debuggable.** Run the check executable directly to reproduce issues.
- **No ABI compatibility.** No shared library versioning concerns.

The overhead of spawning a process is negligible relative to the work checks typically
do (parsing files, running tools, querying APIs).

### Why not WASM plugins

WASM would provide sandboxed, portable, language-agnostic execution without subprocess
overhead. Rejected for now because:

- The WASM toolchain for this kind of tooling is still maturing.
- Subprocess overhead is not a bottleneck.
- WASM adds significant complexity to the engine (runtime embedding, memory management,
  host function bindings).
- Can be added later as an optimization tier if subprocess performance becomes a real
  constraint. YAGNI.

### Why not dynamic library loading (FFI)

Shared libraries (.so/.dylib/.dll) would allow checks written in C/C++/Rust to load
directly into the engine process. Rejected because:

- Ties extensibility to C ABI-compatible languages.
- Introduces safety risks (memory corruption in a check affects the engine).
- Platform-specific binary distribution.
- The subprocess model is simpler and more portable.

### Why not embedded scripting (Lua, Rhai)

An embedded scripting language would allow checks to be written as scripts evaluated by
the engine. Rejected because:

- Adds a language to learn that isn't the user's language of choice.
- Limited ecosystem compared to writing checks in Python, JS, Go, etc.
- Performance characteristics are unpredictable.
- The subprocess model lets users write checks in whatever language they already know.

### Invocation protocol is deferred

The exact wire protocol for external executables (how input is passed, how executables
are registered in check definitions, exit code semantics) is an implementation decision
for Phase 2. This ADR establishes the subprocess + JSON stdout model as the
extensibility mechanism, not the protocol details.

## Consequences

- **Contributor accessibility is language-agnostic.** The barrier to writing a check is
  "produce JSON conforming to ADR-0001," not "learn Rust." This directly mitigates the
  contributor learning curve consequence from ADR-0003.
- **Subprocess overhead per check invocation.** Acceptable for the expected workload.
  Parallel check execution can amortize this.
- **Schema validation at boundaries.** Scute must validate external check output,
  adding runtime validation that native checks don't need.
- **The invocation protocol needs a dedicated design.** How input is passed to external
  executables, how they are registered, and exit code semantics are deferred to Phase 2.
