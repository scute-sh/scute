# ADR-0001: Check Evaluation Schema

**Status:** Accepted
**Date:** 2026-02-26
**Revisions:**
- 2026-03-01 — introduced CheckOutcome wrapper with mutually exclusive
  evaluation/error branches; renamed inner result to Evaluation; renamed Run Envelope
  to WorkflowOutcome.
- 2026-03-02 — removed WorkflowOutcome; aggregation of multiple check outcomes is an
  unsolved problem that needs proper design (see
  [ADR-0006](0006-atomic-checks-and-check-report.md)).
- 2026-03-03 — replaced CheckOutcome wrapper with a flat Evaluation type that takes
  two shapes based on `status` (completed or errored); removed baseline and delta.

## Context

The evaluation schema is the contract between check implementations and their
consumers: coding agents, CI pipelines, CLIs, editors, and MCP servers. All
consumers receive the same structured data. Reporters (CLI formatters, MCP tool
responses, CI summaries) transform it for their context.

Design constraints:

- **Token-efficient** — dense enough that agents can parse, reason, and act without
  burning through their context window.
- **Unambiguous** — CI needs a clear gate signal, not a score to interpret.
- **Numeric** — trend tracking needs values that can be compared over time.
- **Forward-guiding** — every output, whether a fitness evaluation or an execution
  error, must tell the consumer what to do next.

A check that evaluates code and finds violations is fundamentally different from a
check that fails to execute. Both happen in practice. The schema must represent both
without conflating them (see [ADR-0004](0004-check-implementation-architecture.md)).

The fitness function model from *Building Evolutionary Architectures* (Ford,
Parsons, Kua, Sadalage — 2nd ed.) informed the evaluation/error distinction and
the atomic measurement approach.

## Decision

### Evaluation

The per-target output of a check. An Evaluation takes one of two shapes, based on
`status`:

- **Completed** (status: pass, warn, or fail) — the check measured the target and
  produced a verdict.
- **Errored** (status: error) — the check could not evaluate this target.

```
CompletedEvaluation {
  target:      string
  status:      "pass" | "warn" | "fail"
  measurement: Measurement
  evidence?:   Evidence[]
}

ErroredEvaluation {
  target:      string
  status:      "error"
  error:       ExecutionError
}
```

The check name is not part of Evaluation. It lives on the CheckReport
([ADR-0006](0006-atomic-checks-and-check-report.md)), which wraps evaluations
for consumers.

A completed evaluation:

```json
{
  "target": "validate_user (src/auth/login.rs:42)",
  "status": "warn",
  "measurement": { "observed": 12, "thresholds": { "warn": 10, "fail": 15 } }
}
```

An errored evaluation:

```json
{
  "target": "src/broken.rs",
  "status": "error",
  "error": {
    "code": "parse_error",
    "message": "Failed to parse file",
    "recovery": "Fix syntax errors in src/broken.rs"
  }
}
```

### Measurement

```
Measurement {
  observed:   number
  thresholds: Thresholds
}

Thresholds {
  warn?: number
  fail?: number
}
```

### Evidence

```
Evidence {
  rule?:     string       // stable identifier scoped to the check
  location?: string       // where in the target (file:line, path)
  found:     any          // what triggered this finding
  expected?: any          // what the check wanted instead
}
```

### ExecutionError

Describes why a check could not evaluate a target, or why a check could not run
at all (see [ADR-0006](0006-atomic-checks-and-check-report.md) for check-level
errors). Every error includes a `recovery` field so the consumer knows exactly
what to do next.

```
ExecutionError {
  code:      string     // broad, stable category (see ADR-0005)
  message:   string     // human-readable context
  recovery:  string     // actionable next step
  location?: string     // where the problem is (e.g., config file position)
}
```

The error code taxonomy is defined in
[ADR-0005](0005-cli-output-contract.md).

## Design Decisions

### `status` tells the consumer what to do

`status` is the single field consumers branch on. Four values, four actions:

| Status | Action |
|---|---|
| `pass` | Continue |
| `warn` | Investigate |
| `fail` | Stop and fix |
| `error` | Fix the environment |

Completed and errored evaluations carry different fields: completed evaluations
have measurement and evidence, errored evaluations have an ExecutionError. Each
shape has exactly the fields it needs. No null fields, no guessing.

### Why `evaluation`, not `result`

`result` is the most overloaded word in programming. `evaluation` is precise: the
check *evaluated* code against thresholds and produced a verdict. It pairs naturally
with the existing vocabulary: you *define* a check
([ADR-0002](0002-check-definition-format.md)), the check takes a *measurement*
(`observed`), and produces an *evaluation* (status + evidence).

### `recovery` as forward guidance

Every execution error includes a `recovery` field. This is not optional. The whole
point of structured errors is that the consumer can act on them. An error without
guidance is just a fancier crash.

For fitness evaluations, `expected` in evidence serves the same purpose: it tells
the agent what the correct value should be. `recovery` is the error-side equivalent,
but it's an instruction ("install cargo-outdated") rather than a value
("feat, fix, chore"). Different names because they're different in kind.

### Error codes are broad and stable

`code` in ExecutionError is a broad category, not a translation of every possible
upstream error. Scute wraps many external tools; translating each tool's error
messages into scute-branded error codes would be a maintenance trap. The `code` tells
you *what kind* of problem (missing tool, bad config, invalid target). The `message`
carries the specifics.

### `measurement` groups observed value and thresholds

Every check *observes* something and compares it to *thresholds*. The `measurement`
object groups these together so all trend-relevant data lives in one place.

`observed` is the check's measurement, in whatever unit is natural to it. The schema
doesn't prescribe the unit; the check does. Coverage observes a percentage. Layer
violations observe a count, so thresholds like `{ warn: 3, fail: 10 }` allow
controlled tolerance during migrations. Commit-message validity observes 0 or 1,
because the check's scope is a single message.

`thresholds` is always the *resolved* threshold for this evaluation, whether it came
from static config or dynamic computation (e.g., acceptable latency shifting with
concurrent user count).

### `evidence` carries what went wrong

Each evidence item describes a single violation. `found` is what triggered it: the
import that violated a layer rule, the identifier that broke a naming convention,
the cycle path. `rule` identifies which rule was violated. `location` pinpoints where.

`expected` carries what the check wanted instead, when the rule name alone isn't
enough to act on. For example, an `unknown-type` violation includes the list of valid
types so the agent can fix it without external lookup. Self-explanatory rules like
`body-separator` omit `expected` — the name says it all.

### `rule` is an identifier, not prose

Within `evidence`, `rule` is a stable identifier scoped to the check (e.g.,
`"camelCase"`, `"no-persistence-import"`), not a human-readable description (e.g.,
"functions must use camelCase").

Prose descriptions belong in the check *definition*, not repeated in every evidence
item. This keeps evaluations token-efficient, which matters when a naming convention
check produces hundreds of violations. Human-readable text is a reporter concern.

For single-rule checks (like cyclomatic-complexity), `rule` is absent from evidence.
The check *is* the rule.

### `target` identifies what was evaluated

`target` identifies the specific thing this Evaluation assessed: a function, a
dependency, a commit message. Each Evaluation covers one target
([ADR-0006](0006-atomic-checks-and-check-report.md)).

### `observed` is always a number

`observed` is a number. Dependency freshness measures "major versions behind."
Coverage measures percentage. Violations measure count. All check types tested so
far map naturally to a numeric value.

If a check genuinely can't express its measurement as a number, we'll revisit.

### Truncation is a reporter concern

The schema carries complete data. How much gets surfaced is a reporter decision
(MCP might limit evidence items for token budget, CI might show only the summary).
Presentation logic belongs at the edges, not in the schema.

## Alternatives Considered

### Different schemas per check type

Instead of a uniform schema, have different evaluation shapes for metric checks,
violation checks, assertion checks, etc. Rejected because it fragments the consumer
interface. An agent would need to handle N schemas instead of one. The uniform
`measurement` + `evidence` pattern handles all current check types without
branching on check type.

### Human-readable `message` field on evaluations

A `message` field for human consumption (e.g., "Cyclomatic complexity 23 exceeds
fail threshold 20") is redundant. Every piece of information it would contain is
already present in structured fields. Human-readable output is constructed by
reporters from the structured data.

### Wrapper type with two optional branches

A wrapper type with two optional fields (`evaluation?: Evaluation` and
`error?: ExecutionError`), where exactly one is present. Replaced by the flat
two-shape Evaluation because:

- One less nesting level for consumers (e.g., `finding.status` instead of
  `finding.evaluation.status`).
- `status` is the natural branching field — consumers already act on it.
- The wrapper added a type without adding information.

### Separate error schema, not part of Evaluation

Having evaluations and execution errors as completely separate, unrelated
structures. Rejected because consumers need a single type to handle any per-target
result. The two-shape Evaluation makes this natural: one type, `status` as the
branching field, same handling code everywhere.

## Consequences

- Check authors must express their measurement as a number in `observed`. Checks that
  produce fundamentally non-numeric results will need to find a numeric proxy (count
  of violations, versions behind, etc.) or the schema will need revision.
- Reporters (CLI, MCP, CI) are responsible for rendering human-readable output from
  structured data. The schema provides no convenience fields for display.
- The `evidence` array can grow large for high-volume checks. Reporters must handle
  this gracefully.
- The check definition format ([ADR-0002](0002-check-definition-format.md)) must carry
  prose descriptions for rule identifiers referenced in evidence.
- Every execution error must include a `recovery` field. Check authors cannot emit
  an error without actionable guidance.
- The `code` taxonomy for execution errors is defined in
  [ADR-0005](0005-cli-output-contract.md) and must remain broad and stable. Adding a
  new code is a deliberate decision, not a per-tool translation.
