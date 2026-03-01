# ADR-0005: CLI Output Contract

**Status:** Accepted
**Date:** 2026-03-01

## Context

Scute's primary consumers are coding agents, followed by human developers and CI
pipelines. All three need structured, predictable output they can parse and act on.

Before this decision, successful checks produced JSON on stdout while errors
produced unstructured text on stderr. This meant consumers needed two parsing paths,
error messages leaked internal tool details ("cargo outdated failed: error: could
not find `Cargo.toml`..."), and there was no forward guidance on how to recover.

The check outcome schema ([ADR-0001](0001-check-result-schema.md)) defines *what*
the data looks like. This ADR defines *how* it reaches the consumer.

## Decision

### Output streams

**stdout** carries all structured output: CheckOutcomes, WorkflowOutcomes, and
engine-level errors. Always JSON. A consumer reads one stream and always gets
parseable data.

**stderr** is reserved for human-facing diagnostics: verbose/debug logging, progress
indicators, deprecation warnings. Agents ignore it. It is never JSON.

### Engine errors

When the engine itself fails before any check can execute (e.g., unparseable config
file, invalid CLI usage), it emits the same ExecutionError shape
([ADR-0001](0001-check-result-schema.md)) on stdout, just without `check`/`target`
context since that information isn't available yet:

```json
{
  "error": {
    "code": "invalid_config",
    "message": "Failed to parse .scute.yml: expected a map for 'checks', found a sequence",
    "recovery": "Fix the YAML syntax in .scute.yml: 'checks' must be a map, not a list"
  }
}
```

Same error structure, different scope. A check-level error lives inside a
CheckOutcome (with `check` and `target`). An engine error lives at the root
(without them). Consumers parse one error shape either way.

### Exit codes

| Code | Meaning | stdout contains |
|------|---------|----------------|
| 0 | Check passed or warned | CheckOutcome with evaluation |
| 1 | Check failed | CheckOutcome with evaluation |
| 2 | Execution error | CheckOutcome with error, or engine error |

For workflows: exit 0 if all checks pass/warn, exit 1 if any check fails, exit 2
if any check errors (error takes precedence over fail).

### Error code taxonomy

Error codes are coarse categories, not per-tool translations. They are stable
identifiers that consumers can match on programmatically.

| Code | When | Example recovery |
|------|------|-----------------|
| `invalid_target` | Target path doesn't exist or isn't a valid project for this check | "Point to a directory containing a Cargo.toml" |
| `invalid_config` | `.scute.yml` can't be parsed or contains invalid values | "Fix the YAML syntax in .scute.yml" |
| `missing_tool` | A required external tool is not installed | "Install cargo-outdated: cargo install cargo-outdated" |
| `tool_failed` | An external tool ran but crashed or produced unusable output | "Run cargo outdated manually to diagnose the issue" |

New codes are added deliberately when a genuinely new category of failure emerges.
They are never 1:1 translations of upstream tool errors.

## Design Decisions

### stdout for everything, not stderr for errors

The first consumer is a coding agent. Agents capture stdout as the primary channel.
Some agent frameworks don't reliably surface stderr. If engine errors go to stderr,
the agent reads stdout, gets nothing, and has to know to check a second stream for
a different reason. That's a bad contract.

stdout for all structured output means one stream, one parse, one contract.
The JSON shape (CheckOutcome vs naked error) tells the consumer what it got. No
stream-based branching needed.

stderr becomes what it should be: a side channel for human diagnostics that doesn't
interfere with structured data.

### Engine errors vs check errors

Both use the same ExecutionError shape
([ADR-0001](0001-check-result-schema.md)). The difference is scope:

**Engine errors** (no `check`/`target`): the engine failed before it could attempt
any check. Bad config, invalid CLI arguments. The error object sits at the root
of the output.

**Check errors** (scoped to `check`/`target`): a specific check could not execute.
Missing tool, invalid target for that check type. The error object sits inside a
CheckOutcome.

The distinction matters for workflows: an engine error aborts the entire run, while
a check error leaves other checks unaffected.

### Exit code 2 for errors, not exit code 1

Exit 1 means "a check evaluated your code and it failed." Exit 2 means "something
went wrong with the execution itself." CI pipelines and scripts can distinguish
"your code has problems" from "the tooling has problems" without parsing JSON.

This also prevents a common misclassification: a missing tool should not look like
a code quality failure in CI dashboards.

### Error messages don't leak tool internals

`message` describes the problem in scute's terms, not the underlying tool's. "Not a
Cargo project" instead of "cargo outdated failed: error: could not find
`Cargo.toml`". The consumer asked scute to check dependency freshness; they shouldn't
need to know which underlying tool scute uses.

Tool-specific output can appear in verbose/debug stderr for human troubleshooting,
but never in the structured JSON contract.

## Consequences

- All CLI error paths must produce structured JSON on stdout. No unstructured error
  output.
- Error messages require deliberate authoring. Each error needs a clear `message`
  and actionable `recovery`, not a pass-through of upstream tool output.
- The error code taxonomy must be maintained as a stable contract. Adding codes is
  a design decision, not a side effect of wrapping a new tool.
- stderr output (verbose, debug, progress) has no schema and no stability guarantee.
  It's for humans debugging issues, not for programmatic consumption.
- Exit code 2 is a breaking change from current behavior where all errors exit 1.
  Consumers relying on "non-zero means failure" are unaffected; consumers
  distinguishing exit 1 from exit 2 get a new signal.
