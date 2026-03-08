# ADR-0002: Check Definition Format

**Status:** Accepted
**Date:** 2026-02-26

## Context

A check definition declares the desired state for a fitness function: what to
measure and what thresholds to enforce. It is the input-side counterpart to the
check evaluation schema ([ADR-0001](0001-check-evaluation-schema.md)).

Definitions are purely declarative. They describe *what* is acceptable, never *how*
to measure it or *when* to run it.

## Decision

### Check Definition

```yaml
check: cyclomatic-complexity
thresholds:
  warn: 10
  fail: 20
```

```yaml
check: layer-dependency
layers:
  ui: ["src/ui/**"]
  domain: ["src/domain/**"]
  persistence: ["src/db/**"]
deny:
  no-persistence-import:
    description: "UI layer must not import from persistence"
    from: ui
    to: persistence
thresholds:
  fail: 0
```

### Fields

| Field | Required | Purpose |
|---|---|---|
| `check` | yes | Unique identifier. Maps to `CheckReport.check`. For built-in checks, also identifies the measurement implementation. |
| `thresholds` | yes | `{ warn?, fail? }` — at least one must be present. |

`thresholds` is a reserved key. Everything else at the same level is check-specific
configuration, validated by the check implementation.

## Design Decisions

### Purely declarative — no measurement logic

The definition says *what* to check and *what thresholds to enforce*. It never says
*how* to measure. This separation means the same definition format works regardless
of whether the measurement is built-in, wraps an external tool, or calls a remote
service.

### No `scope` field

Scope (which files, modules, or services to check) is an invocation concern, not a
definition property. The same check definition applies identically whether invoked
by:

- An agent checking a specific file
- A pre-commit hook checking staged files
- A PR gate checking changed files in the branch
- A scheduled pipeline checking everything

Scope is determined at invocation time. The definition declares desired state
independent of invocation context.

### `check` is both identity and implicit type

For built-in checks, the name identifies both the check instance and its
measurement implementation. No separate `type` field is needed. If a future need
arises for multiple instances of the same check type with different configs (e.g.,
strict-complexity for core code, relaxed-complexity for legacy), an explicit `type`
field can be added that defaults to `check` when absent.

### Check-specific options live alongside `thresholds`

There is no separate `config` wrapper. Check-specific options are siblings of
`thresholds` at the same YAML level. The definition schema does not interpret
these options; they are passed through to the check implementation, which
validates its own configuration. `thresholds` is the only reserved key.

Rule IDs (referenced in `Evidence.rule` in the check evaluation schema) appear as keys
in the definition. The convention is that `description` is a reserved key within any
rule definition, carrying prose for human-readable reporters.

### Absolute and delta checks are separate fitness functions

A check like test coverage can be assessed two ways:

- Absolute: "coverage must stay above 50%"
- Delta: "coverage must not drop by more than 5%"

These are separate checks with separate definitions, not modes of one check. Each
has its own `observed` value (absolute percentage vs. change in percentage), its
own thresholds, and produces its own `status`.

```yaml
check: test-coverage
thresholds:
  warn: 70
  fail: 50
```

```yaml
check: test-coverage-delta
thresholds:
  warn: -5
  fail: -15
```

This keeps the evaluation simple: one `observed`, one set of thresholds, one
`status`. No branching logic to determine which criterion triggered the verdict.

### Threshold direction is implicit

For some checks, higher is worse (complexity: warn at 10, fail at 20). For others,
lower is worse (coverage: warn at 70, fail at 50). The direction is inferrable from
the relationship between `warn` and `fail`: when `warn < fail`, higher is worse;
when `warn > fail`, lower is worse. No explicit direction field is needed.

### No cadence, invocation, or proactivity fields

The fitness function taxonomy (*Building Evolutionary Architectures*) defines
categories for when and how a check runs: triggered vs. continual, automated vs.
manual, intentional vs. emergent. These are deployment concerns, not definition
properties. The same check definition can be triggered in CI, continual in
production monitoring, and invoked manually during exploratory testing.

## Consequences

- Check implementations must validate their own options. The definition schema
  provides no type safety for check-specific configuration.
- Teams that want both absolute and delta checks for the same metric must define
  two checks. This is intentional. Explicit over implicit.
- The `description` convention within rule definitions is a soft contract. Nothing
  enforces that rule definitions include descriptions.
- Without `scope` in the definition, scope must be provided at invocation time
  (CLI flags, MCP parameters, hook context, etc.).
