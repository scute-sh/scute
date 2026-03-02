# ADR-0006: Atomic Checks and Target Resolution

**Status:** Draft
**Date:** 2026-03-02

## Context

Building the cyclomatic-complexity check revealed a modeling gap. The existing
checks (commit-message, dependency-freshness) conflate two concerns: evaluating a
target and selecting which targets to evaluate. Dependency-freshness takes a
directory, finds all outdated deps, and reports a count. The `observed` value is the
number of violations, not a property of any single dependency.

This works, but it doesn't fit checks where the measurement is inherently about one
target. Cyclomatic complexity is a property of a single function. The meaningful
`observed` value is the complexity score, not "how many functions are too complex."
Thresholds should apply to the score ("warn at 10, fail at 15"), not to a count of
violations.

The same argument applies to dependency freshness in hindsight: the meaningful
measurement is how far behind a single dependency is (the version gap), not how many
deps are outdated. The count-of-violations model loses the per-target signal that
makes the check actionable.

This ADR names the concepts that emerged from this analysis and establishes the
direction for future check design.

## Decision

### Checks are atomic evaluations

A check evaluates **one target** and produces **one CheckOutcome**. The `observed`
value is a direct property of that target.

- Cyclomatic complexity: target = function, observed = complexity score
- Dependency freshness: target = dependency, observed = version gap
- Commit message: target = message, observed = violation count (0 or 1)

The check knows nothing about where the target came from or how many siblings exist.

### Target kinds

Each check operates on a specific **target kind**: function, dependency, commit
message. The target kind determines what the check expects as input and what
extraction means.

Multiple checks can share a target kind. Cyclomatic complexity, cognitive complexity,
nesting depth, and function length all operate on functions. This matters because
extraction is per-target-kind, not per-check.

### Target resolution is a separate concern

Selecting which targets to evaluate is orthogonal to the evaluation itself. This
concern decomposes into:

**Scope** — a user-specified boundary: a file, a directory, staged changes, a PR
diff, a specific function by name.

**Extraction** — given a scope, produce a list of targets of a given kind. Parsing a
file to find functions is extraction. Reading a manifest to find dependencies is
extraction. Extraction is per-target-kind, reusable across checks.

The composition:

```
scope → extract(target_kind) → [target₁, target₂, ...] → map(check) → [outcome₁, outcome₂, ...]
```

### The API hides the machinery

Users don't think in terms of scope, extraction, and target kinds. They point at
things:

```
scute check cyclomatic-complexity src/config.rs        # file
scute check cyclomatic-complexity src/                  # directory
scute check cyclomatic-complexity --staged              # git context
```

Scute figures out what the user pointed at, extracts the appropriate targets, and
runs the check on each. The composition model is internal architecture, not
user-facing vocabulary.

### Aggregation is an unsolved problem

When a scope produces 20 functions, the check runs 20 times and produces 20
CheckOutcomes. How those results are collected, summarized, and presented to the
user is a separate design problem. It matters for CLI output, MCP responses, CI
gates, and trend storage. But it should be solved when we have real usage to guide
it, not designed speculatively now.

## Implications for Existing Checks

Dependency-freshness currently uses the aggregate pattern: observed = count of
outdated deps. Under the atomic model, it would become: observed = version gap of
one dependency, with each dep producing its own CheckOutcome. This is a cleaner
design but not an urgent migration. The existing implementation works and can be
revisited when the target resolution layer materializes.

## What This ADR Does NOT Decide

- **Scope resolution details.** How scopes are specified, how git-aware scoping
  works, how the CLI maps arguments to scopes. That's a future design concern.
- **Extraction implementation.** Whether extraction lives in the check, in a shared
  library, or in a separate layer. Let usage drive the shape.
- **Migration of existing checks.** Dependency-freshness works today. When and how
  to migrate it to the atomic model is a separate decision.
- **Aggregation semantics.** How per-target outcomes are summarized (worst status?
  count of failures?) is a reporter/workflow concern, not a check concern.

## Relationship to Other ADRs

- **ADR-0001 (Check Outcome Schema):** The CheckOutcome schema supports atomic
  checks without changes. The `target` field narrows from "scope boundary" to "the
  specific thing being evaluated." One tension: ADR-0001's design decision that "a
  single check outcome carries all violations in one evaluation" assumes the batch
  model. Atomic checks produce one outcome per target instead. Both models produce
  valid CheckOutcome values; the difference is granularity. Aggregation of multiple
  outcomes is deferred.
- **ADR-0004 (Check Implementation Architecture):** Unchanged. The three tiers
  (native, subprocess wrapper, external executable) apply regardless of whether a
  check is atomic or aggregate.
