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
3. **Extensibility without the engine's language.** Requiring contributors to learn
   the engine's implementation language to write a custom check would limit adoption.
   The check outcome schema ([ADR-0001](0001-check-result-schema.md)) is
   language-agnostic, so the implementation architecture should be too.

## Decision

Three tiers of check implementation. All three produce the same CheckOutcome
([ADR-0001](0001-check-result-schema.md)). Consumers don't know or care which tier
generated it.

### Tier 1: Native

Built into the engine for checks where performance or deep AST access matters.

Examples: cyclomatic complexity, circular dependency detection, layer violation
analysis, naming convention enforcement.

### Tier 2: Subprocess Wrapper

Engine glue code that invokes an external tool, parses its output, and normalizes
it into the check outcome schema. The engine code is thin glue — the external tool
does the heavy lifting.

Examples:
- Dependency freshness → wraps `npm outdated --json`, `cargo outdated --format json`
- Test coverage → wraps `jest --coverage --json`, `cargo llvm-cov --json`
- Known vulnerabilities → wraps `cargo audit --json`, `npm audit --json`

The wrapper handles:
- Tool invocation and exit code interpretation
- Output parsing (JSON, XML, plaintext — whatever the tool emits)
- Mapping to `observed` + `evidence`
- Applying `thresholds` from the check definition
  ([ADR-0002](0002-check-definition-format.md))

### Tier 3: External Executable

Any executable that conforms to a check protocol. For custom, proprietary, or
domain-specific checks written in any language.

The contract: scute provides the check definition and target as input, the
executable produces a CheckOutcome ([ADR-0001](0001-check-result-schema.md)) as JSON
on stdout. Scute validates it at the boundary. The exact invocation protocol (how
input is passed, how the executable is registered) is deferred.

What matters at the architecture level:

- **Any language.** A Python script, a shell script, a Go binary — anything
  executable.
- **Separation of check status from execution status.** A check that executes
  successfully but finds violations is not the same as a check that fails to run.
  This is realized through the CheckOutcome wrapper: mutually exclusive `evaluation`
  (fitness assessed) or `error` (execution failed). Tier 2 and Tier 3 checks
  produce evaluations; scute catches execution failures at the boundary and wraps
  them as errors.

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

### Invocation protocol is deferred

The exact wire protocol for external executables (how input is passed, how executables
are registered in check definitions, exit code semantics) is an implementation
decision. This ADR establishes the subprocess + JSON stdout model as the extensibility
mechanism, not the protocol details.

## Alternatives Considered

### WASM plugins

WASM would provide sandboxed, portable, language-agnostic execution without subprocess
overhead. Rejected for now because:

- The WASM toolchain for this kind of tooling is still maturing.
- Subprocess overhead is not a bottleneck.
- WASM adds significant complexity to the engine (runtime embedding, memory management,
  host function bindings).
- Can be added later as an optimization tier if subprocess performance becomes a real
  constraint. YAGNI.

### Dynamic library loading (FFI)

Shared libraries would allow checks written in native languages to load directly into
the engine process. Rejected because:

- Ties extensibility to C ABI-compatible languages.
- Introduces safety risks (memory corruption in a check affects the engine).
- Platform-specific binary distribution.
- The subprocess model is simpler and more portable.

### Embedded scripting

An embedded scripting language would allow checks to be written as scripts evaluated
by the engine. Rejected because:

- Adds a language to learn that isn't the user's language of choice.
- Limited ecosystem compared to writing checks in Python, JS, Go, etc.
- Performance characteristics are unpredictable.
- The subprocess model lets users write checks in whatever language they already know.

## Consequences

- **Contributor accessibility is language-agnostic.** The barrier to writing a check is
  "produce JSON conforming to [ADR-0001](0001-check-result-schema.md)," not "learn the
  engine's language." This directly mitigates the contributor learning curve consequence
  from [ADR-0003](0003-language-choice.md).
- **Subprocess overhead per check invocation.** Acceptable for the expected workload.
  Parallel check execution can amortize this.
- **Schema validation at boundaries.** Scute must validate external check output,
  adding runtime validation that native checks don't need.
- **The invocation protocol needs a dedicated design.** How input is passed to external
  executables, how they are registered, and exit code semantics are deferred.
