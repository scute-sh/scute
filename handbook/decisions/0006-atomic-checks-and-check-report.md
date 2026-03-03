# ADR-0006: Atomic Checks and Check Report

**Status:** Accepted
**Date:** 2026-03-02

## Context

Checks measure properties of individual targets. Cyclomatic complexity is a
property of one function. Dependency freshness is a property of one dependency.
The meaningful `observed` value is a direct measurement of that target (a
complexity score, a version gap), not an aggregate like "how many targets have
problems."

At the same time, a single check invocation often evaluates many targets. A
cyclomatic-complexity check pointed at a source file evaluates every function in
it. A commit-message check pointed at a PR evaluates every commit. The check
needs to return per-target results, and consumers need to receive them in a shape
that is both complete and token-efficient.

Only the check knows how to interpret its input. A file path means "find functions"
to cyclomatic-complexity but "read the manifest" to dependency-freshness. A commit
message string is already the target. This interpretation cannot be extracted into
a generic layer without coupling it to check-specific knowledge. For custom checks
([ADR-0004](0004-check-implementation-architecture.md)), keeping the contract to
a single script is essential: one executable that takes input and returns results.

## Decision

### Evaluations are atomic

An Evaluation ([ADR-0001](0001-check-evaluation-schema.md)) assesses **one target**.
The `observed` value is a direct property of that target.

| Check | Target | Observed |
|---|---|---|
| Cyclomatic complexity | function | complexity score |
| Dependency freshness | dependency | version gap |
| Commit message | message | violations found (0 or 1) |

### The check decides what to evaluate

The check takes input, decides what targets it contains, evaluates each one, and
returns the results. Some inputs are already a single target (one commit message).
Others contain many (all functions in a file, all dependencies in a project).

The check returns a single Evaluation or a list, whichever matches what it
evaluated. Check authors return evaluations, nothing more.

### Scute wraps results in a CheckReport

Scute accepts whatever the check returns (single or list), and normalizes it into
a consistent shape for consumers:

```
CheckReport {
  check:      string
  summary?:   Summary          // present when the check ran
  findings?:  Evaluation[]     // present when the check ran
  error?:     ExecutionError   // present when the check could not run
}

Summary {
  evaluated: number
  passed:    number
  warned:    number
  failed:    number
  errored:   number
}
```

`summary + findings` and `error` are mutually exclusive, mirroring the same
two-shape pattern inside Evaluation ([ADR-0001](0001-check-evaluation-schema.md)):
a check either ran (producing evaluations) or it didn't (producing an error).

**check** — which check produced these results.

**summary + findings** — present when the check ran, even if every target
failed evaluation. `summary` is computed by Scute from all evaluations and
carries what `findings` alone cannot: how many targets were evaluated, how
many passed. `findings` contains non-passing evaluations only (warn, fail,
error). Passing results are excluded to keep payloads token-efficient for
agents.

**error** — present when the check could not run at all: a missing external
tool, an unreadable config file, an invalid project path. The check never
reached target evaluation, so there are no findings and no summary.

A target-level error (one file couldn't be parsed) is an Evaluation with
`status: "error"` in `findings`. A check-level error (the tool isn't
installed) is an `error` on the report itself. Different failure, different
recovery path.

The report is Scute infrastructure. Check authors never construct it.

## Consequences

- The check contract stays simple: take input, return Evaluation(s). This holds
  across all three implementation tiers
  ([ADR-0004](0004-check-implementation-architecture.md)).
- Consumers always receive a CheckReport, whether the check evaluated one target
  or a thousand. One shape to parse.
- Passing evaluations are excluded from `findings`. If consumers need full results
  (e.g., for trend tracking), the filtering behavior may need to become
  configurable.
- Dependency-freshness currently returns an aggregate count instead of
  per-dependency evaluations. It can migrate to the atomic model independently.
- CLI stdout ([ADR-0005](0005-cli-output-contract.md)) carries CheckReports.

## Relationship to Other ADRs

- **ADR-0001 (Check Evaluation Schema):** Evaluation remains the atomic unit,
  unchanged. CheckReport wraps Evaluations for consumers; it does not replace
  or modify them.
- **ADR-0004 (Check Implementation Architecture):** The three implementation tiers
  apply unchanged. Custom checks return Evaluation(s) as JSON on stdout; Scute
  wraps them in the report.
- **ADR-0005 (CLI Output Contract):** stdout carries CheckReports.
