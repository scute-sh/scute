# ADR-0003: Implementation Language

**Status:** Accepted
**Date:** 2026-02-27

## Context

Scute is a CLI-first tool that runs in git hooks, coding agent feedback loops, editor
integrations, CI pipelines, and production monitors. The implementation language must
satisfy constraints that stem directly from the project's design principles and usage
patterns.

**Startup time is a UX requirement.** A git hook fires on every commit. An agent tool
call happens mid-generation. An editor integration runs on save. These contexts demand
sub-100ms cold start. Anything slower introduces friction that degrades adoption.

**Single binary distribution.** Privacy-first (no cloud, no telemetry) means the tool
must run entirely on the user's machine with no runtime dependency. A self-contained
binary eliminates "install Node/Python/Go first" as a prerequisite and simplifies
CI caching, container images, and air-gapped environments.

**Type system alignment with the check contract.** The check result schema (ADR-0001)
is the product. `status: pass | warn | fail` is a sum type. `observed` is always
numeric. `evidence` is an optional array of structured items. The implementation
language should enforce this contract at compile time, not through runtime validation
and convention.

**Multi-language code parsing.** Built-in checks (cyclomatic complexity, circular
dependencies, layer violations) require parsing source code across many languages.
tree-sitter is the industry standard for incremental, multi-language parsing.

**Cross-platform.** Developers and CI environments run Linux, macOS, and Windows. The
toolchain must produce native binaries for all three without heroics.

## Decision

Rust.

## Design Decisions

### Startup time

Rust produces native binaries with no runtime initialization overhead. Cold start is
sub-millisecond. For a tool that may run multiple checks per commit, this compounds.

### Type system enforces the check contract

ADR-0001's check result schema maps directly to Rust's type system:

- `status: pass | warn | fail` → `enum Status { Pass, Warn, Fail }`
- `observed: number` → `f64`
- `expected: { warn?, fail? }` → `struct Expected { warn: Option<f64>, fail: Option<f64> }`
- `evidence?: [...]` → `Option<Vec<Evidence>>`

Check authors get schema violations at compile time. Serde handles JSON serialization
and YAML deserialization with derive macros, making the contract enforceable with
minimal boilerplate.

### tree-sitter alignment

tree-sitter is written in C with first-class Rust bindings (`tree-sitter` crate) —
the most mature bindings across all language ecosystems.

### CLI ecosystem is sufficient

Scute's primary output is structured JSON consumed by agents, CI, and MCP. The CLI's
human-readable mode is a thin formatting layer over the same data. `clap` handles
argument parsing. Terminal formatting libraries (`owo-colors`, `console`) handle the
reporter. No interactive TUI is needed — Scute is not that kind of tool.

### MCP server

No official Rust MCP SDK exists. The protocol is JSON-RPC over stdio, and the surface
Scute needs (tool registration, structured input/output) is small. Community crates
exist; implementing directly is also straightforward.

## Alternatives Considered

### Go

Fast startup (~5-10ms), single binary, trivial cross-compilation, simple language with
a broad contributor base. Strong CLI ecosystem (cobra, kong).

Rejected because: Go's type system cannot express the check result schema as precisely.
No sum types means `status` becomes a string with runtime validation. Error handling
verbosity (`if err != nil`) accumulates across many check implementations.
tree-sitter bindings are second-class. The simplicity advantage is real but is
offset by ADR-0004's executable protocol, which makes contributor accessibility
language-agnostic.

### TypeScript (Node / Deno / Bun)

Largest potential contributor pool. MCP SDK is TypeScript-first. Native JS/TS parsing.
Fastest development velocity for early phases.

Rejected because: Startup time disqualifies it for git hooks and agent tool calls.
Runtime dependency (Node) or large compiled binary (Deno ~80MB+) conflicts with
single-binary distribution. npm's dependency model conflicts with "minimal
dependencies" and "secure." The performance ceiling is lower for heavy computation
across large codebases.

## Consequences

- **Steeper contributor learning curve** for the core engine. Mitigated by ADR-0004's
  executable protocol, which allows check implementations in any language.
- **Slower Phase 1 development velocity** compared to Go or TypeScript. Accepted as a
  tradeoff for long-term correctness and performance.
- **Compile times** add development friction. Incremental compilation and workspace
  structure will need attention as the codebase grows.
- **MCP server must be implemented without an official SDK.** The protocol surface is
  small enough that this is engineering, not risk.
