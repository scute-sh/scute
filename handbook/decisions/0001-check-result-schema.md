# ADR-0001: Check Outcome Schema

**Status:** Accepted
**Date:** 2026-02-26
**Revised:** 2026-03-01 — introduced CheckOutcome wrapper with mutually exclusive
evaluation/error branches; renamed inner result to Evaluation; renamed Run Envelope
to WorkflowOutcome.

## Context

The check outcome schema is the contract between check implementations and their
consumers: coding agents, CI pipelines, CLIs, editors, and MCP servers. All
consumers receive the same structured data. Reporters transform it for their
context.

Design constraints:

- **Token-efficient** — semantically dense so agents can parse, reason, and act in
  minimal tokens.
- **Unambiguous** — CI needs a clear gate signal, not a score to interpret.
- **Numeric** — trend storage needs values that can be compared over time.
- **Forward-guiding** — every output, whether fitness evaluation or execution error,
  must tell the consumer what to do next.

A check that evaluates code and finds violations is fundamentally different from a
check that fails to execute. Both happen in practice. The schema must represent both
without conflating them (see [ADR-0004](0004-check-implementation-architecture.md)).

The fitness function taxonomy from *Building Evolutionary Architectures* (Ford,
Parsons, Kua, Sadalage — 2nd ed.) informed the design, particularly the
static/dynamic result distinction and the atomic/holistic scope distinction.

## Decision

### CheckOutcome

The outcome of invoking a check against a target. Contains either an `evaluation`
(the check executed and produced a fitness assessment) or an `error` (the check
could not execute). Exactly one is present, never both.

```
CheckOutcome {
  check:       string
  target:      string
  evaluation?: Evaluation       // present when the check executed
  error?:      ExecutionError   // present when execution failed
}
```

A successful evaluation:

```json
{
  "check": "dependency-freshness",
  "target": "src/",
  "evaluation": {
    "status": "pass",
    "measurement": { "observed": 0, "thresholds": { "fail": 0 } }
  }
}
```

An execution error:

```json
{
  "check": "dependency-freshness",
  "target": "/tmp",
  "error": {
    "code": "invalid_target",
    "message": "Not a Cargo project",
    "recovery": "Point to a directory containing a Cargo.toml"
  }
}
```

### Evaluation

The core output of a fitness function. When a check executes successfully, it
produces an Evaluation: a verdict, a measurement, and optionally the evidence that
led to that verdict.

```
Evaluation {
  status:      "pass" | "warn" | "fail"
  measurement: Measurement
  evidence?:   Evidence[]
  baseline?:   Baseline
  delta?:      number          // observed - baseline.observed
}

Measurement {
  observed:   number
  thresholds: Thresholds
}

Thresholds {
  warn?: number
  fail?: number
}

Evidence {
  rule?:     string       // stable identifier scoped to the check
  location?: string       // where in the target (file:line, path)
  found:     any          // what triggered this finding
  expected?: any          // what the check wanted instead
}

Baseline {
  observed: number
  commit:   string
}
```

### ExecutionError

Describes why a check could not execute. Every error includes a `recovery` field
so the consumer knows exactly what to do next.

```
ExecutionError {
  code:      string     // coarse, stable category (see ADR-0005)
  message:   string     // human-readable context
  recovery:  string     // actionable next step
  location?: string     // source of the problem (e.g., config file position)
}
```

The error code taxonomy is defined in
[ADR-0005](0005-cli-output-contract.md).

### WorkflowOutcome

When multiple checks run together, their CheckOutcomes are aggregated into a
WorkflowOutcome. A CheckOutcome from `scute check` is structurally identical to a
single item in the `outcomes` array, so consumers parse one shape everywhere.

```
WorkflowOutcome {
  version:   string
  run_id:    string
  timestamp: string          // ISO 8601
  commit?:   string
  outcomes:  CheckOutcome[]
  summary:   Summary
}

Summary {
  pass:  number
  warn:  number
  fail:  number
  error: number
  total: number
}
```

## Design Decisions

### `evaluation` and `error` are mutually exclusive

A CheckOutcome contains exactly one: an evaluation or an error. This is enforced at
the type level, not by convention.

A check that executes successfully and finds 50 violations is an evaluation with
`status: fail`. A check that can't run because the tool is missing is an error. The
consumer's recovery path is completely different: fix your code vs fix your
environment. Conflating them in one structure would force consumers to branch on
implicit signals.

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

### Error codes are coarse and stable

`code` in ExecutionError is a stable category, not a translation of every possible
upstream error. Scute wraps many external tools; translating each tool's error
messages into scute-branded error codes would be a maintenance trap. The `code` tells
you *what kind* of problem (missing tool, bad config, invalid target). The `message`
carries the specifics.

### `measurement` groups observed value and thresholds

Every check *observes* something and compares it to *thresholds*. The `measurement`
object groups these together so all trending-relevant data lives in one place.

`observed` is the check's measurement, in whatever unit is natural to it. The schema
doesn't prescribe the unit; the check does. Coverage observes a percentage. Layer
violations observe a count, so thresholds like `{ warn: 3, fail: 10 }` allow
controlled tolerance during migrations. Commit-message validity observes 0 or 1,
because the check's scope is a single message.

`thresholds` is always the *resolved* threshold for this evaluation, whether it came
from static config or dynamic computation (e.g., acceptable latency shifting with
concurrent user count).

### `status` as three-value enum, not score

`pass | warn | fail`. Not a number, not a percentage, not a letter grade.

Agents need unambiguous signals. "What do I do with 73%?" is a question that wastes
tokens. "fail" is an instruction. The three values map directly to action: continue,
investigate, stop and fix.

### `evidence` as structured findings

Each evidence item describes a single violation. `found` is what triggered it: the
import that violated a layer rule, the identifier that broke a naming convention,
the cycle path. `rule` identifies which rule was violated. `location` pinpoints where.

`expected` carries what the check wanted instead, when the rule name alone isn't
enough to act on. For example, an `unknown-type` violation includes the list of valid
types so the agent can fix it without external lookup. Self-explanatory rules like
`body-separator` omit `expected` — the name says it all.

### `rule` as identifier, not prose

Within `evidence`, `rule` is a stable identifier scoped to the check (e.g.,
`"camelCase"`, `"no-persistence-import"`), not a human-readable description (e.g.,
"functions must use camelCase").

Prose descriptions belong in the check *definition*, not repeated in every evidence
item. This keeps evaluations token-efficient, which matters when a naming convention
check produces hundreds of violations. Human-readable text is a reporter concern.

For single-rule checks (like cyclomatic-complexity), `rule` is absent from evidence.
The check *is* the rule.

### `target` as scope boundary

`target` identifies the scope boundary being checked: a file, module, package, or
service. Specific locations within that scope (line numbers, identifiers) live inside
`evidence[].location`.

This means a single check outcome for "naming-convention on src/billing/reconcile.ts"
carries all violations in one evaluation rather than producing N top-level outcomes.

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

Instead of a uniform schema, have different evaluation shapes for metric checks,
violation checks, assertion checks, etc. Rejected because it fragments the consumer
interface. An agent would need to handle N schemas instead of one. The uniform
`measurement` + `evidence` pattern handles all current check types without type
branching.

### Human-readable `message` field on evaluations

A `message` field for human consumption (e.g., "Cyclomatic complexity 23 exceeds
fail threshold 20") is redundant. Every piece of information it would contain is
already present in structured fields. Human-readable output is constructed by
reporters from the structured data.

### `error` as a fourth status value

Adding `"status": "error"` alongside pass/warn/fail would keep a single branching
field. Rejected because it conflates two fundamentally different things: "the check
measured something" vs "the check couldn't run." An evaluation with `status: fail`
has measurement data, evidence, thresholds. An error has none of that. Forcing both
into one structure with mostly-null fields is worse than a clean union type.

### Separate error schema, not integrated in CheckOutcome

Having check evaluations and execution errors as completely separate, unrelated
structures. Rejected because it fragments the workflow aggregation story. A
WorkflowOutcome needs to collect both successful evaluations and execution errors
in one array. The CheckOutcome wrapper makes this natural.

## Consequences

- Check authors must express their measurement as a number in `observed`. Checks that
  produce fundamentally non-numeric results will need to find a numeric proxy (count
  of violations, versions behind, etc.) or the schema will need revision.
- Reporter implementations (CLI, MCP, CI) are responsible for rendering human-readable
  output from structured data. The schema provides no convenience fields for display.
- The `evidence` array can grow large for high-volume checks. Reporters must handle
  this gracefully.
- The check definition format ([ADR-0002](0002-check-definition-format.md)) must carry
  prose descriptions for rule identifiers referenced in evidence.
- Every execution error must include a `recovery` field. Check authors cannot emit
  an error without actionable guidance.
- The `code` taxonomy for execution errors is defined in
  [ADR-0005](0005-cli-output-contract.md) and must remain coarse and stable. Adding a
  new code is a deliberate decision, not a per-tool translation.
