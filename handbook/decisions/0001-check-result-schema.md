# ADR-0001: Check Result Schema

**Status:** Accepted
**Date:** 2026-02-26

## Context

The check result schema is the contract between check implementations and their
consumers: coding agents, CI pipelines, CLIs, editors, and MCP servers. All
consumers receive the same structured data. Reporters transform it for their
context.

Design constraints:

- **Token-efficient** — agents must parse failures and act in minimal tokens.
- **Unambiguous** — CI needs a clear gate signal, not a score to interpret.
- **Numeric** — trend storage needs values that can be compared over time.
- **Agent-readable** — semantically dense so an LLM can reason and act in minimal tokens.

The fitness function taxonomy from *Building Evolutionary Architectures* (Ford,
Parsons, Kua, Sadalage — 2nd ed.) informed the design, particularly the
static/dynamic result distinction and the atomic/holistic scope distinction.

## Decision

### Check Result (single check, single target)

```json
{
  "check": "string",
  "target": "string",
  "status": "pass | warn | fail",
  "observed": "number",
  "expected": { "warn?": "number", "fail?": "number" },
  "evidence?": [{ "location?": "string", "rule?": "string", "found": "any" }],
  "baseline?": { "observed": "number", "commit": "string" },
  "delta?": "number"
}
```

### Run Envelope (aggregates multiple check results)

```json
{
  "version": "string",
  "run_id": "string",
  "timestamp": "ISO 8601",
  "commit?": "string",
  "results": [/* check results */],
  "summary": { "pass": "number", "warn": "number", "fail": "number", "total": "number" }
}
```

## Design Decisions

### `observed` + `expected`

Every check *observes* something and compares it to an *expectation*. For metrics,
`observed` is a measurement and `expected` carries thresholds. For violations,
`observed` is a count and `expected` is `{ fail: 0 }`. The abstraction holds across
check types.

`expected` is always the *resolved* threshold for this evaluation, whether it came
from static config or dynamic computation (e.g., acceptable latency shifting with
concurrent user count).

### `status` as three-value enum, not score or percentage

`pass | warn | fail`. Not a number, not a percentage, not a letter grade.

Agents need unambiguous signals. "What do I do with 73%?" is a question that wastes
tokens. "fail" is an instruction. The three values map directly to action: continue,
investigate, stop and fix.

### `evidence` as agent-readable structured findings

Each evidence item is a structured object with optional `location`, optional `rule`,
and required `found`. `found` carries what was observed at that location: the
import that violated a layer rule, the identifier that broke a naming convention,
the cycle path. An agent uses this to know exactly what to change and where.

### `rule` as identifier, not prose

Within `evidence`, `rule` is a stable identifier scoped to the check (e.g.,
`"camelCase"`, `"no-persistence-import"`), not a human-readable description (e.g.,
"functions must use camelCase").

Prose descriptions belong in the check *definition*, not repeated in every evidence
item. This keeps results token-efficient, which matters when a naming convention check
produces hundreds of violations. Human-readable text is a reporter concern: the CLI
formatter looks up the rule ID and renders the description.

For single-rule checks (like cyclomatic-complexity), `rule` is absent from evidence.
The check *is* the rule.

### `target` as scope boundary, specific locations in evidence

`target` identifies the scope boundary being checked: a file, module, package, or
service. Specific locations within that scope (line numbers, identifiers) live inside
`evidence[].location`.

This means a single check result for "naming-convention on src/billing/reconcile.ts"
carries all violations in one result rather than producing N top-level results. It
keeps the results array manageable and groups related findings logically.

### `observed` as number

`observed` is a number. Dependency freshness measures "major versions behind."
Coverage measures percentage. Violations measure count. All check types tested so
far map naturally to a numeric value.

If a check genuinely can't express its measurement as a number, we'll revisit.

### Truncation is a reporter concern

The schema carries complete data. How much gets surfaced is a reporter decision
(MCP might limit evidence items for token budget, CI might show only the summary).
Presentation logic belongs at the edges, not in the schema.

### Baseline and delta are optional but schema-level

`baseline` and `delta` exist in the schema before a trend store does. Individual
checks can compute deltas against a provided baseline independently. Baking these
in early is cheaper than retrofitting, and agents learn to use directional data
from the start.

## Alternatives Considered

### Typed union per check category

Instead of a uniform schema, have different result shapes for metric checks,
violation checks, assertion checks, etc. Rejected because it fragments the consumer
interface. An agent would need to handle N schemas instead of one. The uniform
`observed` + `expected` + `evidence` pattern handles all current check types without
type branching.

### Human-readable `message` field

A `message` field for human consumption (e.g., "Cyclomatic complexity 23 exceeds
fail threshold 20") is redundant. Every piece of information it would contain is
already present in structured fields. Human-readable output is constructed by
reporters from the structured data.

## Consequences

- Check authors must express their measurement as a number in `observed`. Checks that
  produce fundamentally non-numeric results will need to find a numeric proxy (count
  of violations, versions behind, etc.) or the schema will need revision.
- Reporter implementations (CLI, MCP, CI) are responsible for rendering human-readable
  output from structured data. The schema provides no convenience fields for display.
- The `evidence` array can grow large for high-volume checks. Reporters must handle
  this gracefully.
- The check definition format (ADR-0002) must carry prose descriptions for rule
  identifiers referenced in evidence.
