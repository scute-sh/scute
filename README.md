# Scute

**Define the boundaries. Let your code evolve freely within them.**

An open-source toolkit for deterministic fitness checks across your entire
software delivery lifecycle. Built for developers and coding agents alike.

## The Problem

We check whether code compiles. We check whether tests pass. We rarely check
whether the software itself is heading in the right direction. Are architecture
boundaries holding? Is complexity creeping? Are dependencies drifting? Are we
honoring our SLOs?

Most teams don't lack standards. They lack enforcement that's automated,
measurable, and doesn't rot alongside the codebase.

Two shifts make this more urgent:

1. **Coding agents produce code faster than humans can review it.** "Does it
   work?" is not the same as "is it right?". Agents need clear, automated
   checks to validate both. So do the humans reviewing what agents produce.

2. **Architecture degrades gradually, then suddenly.** A codebase that was
   healthy six months ago may be quietly rotting. Point-in-time snapshots miss
   what matters most: the trend.

Existing tools (CodeScene, SonarQube, Code Climate, etc.) address parts of
this, but they share common limitations: proprietary, expensive at scale,
dashboard-centric, and built for humans clicking through a UI rather than for
a developer's terminal or a coding agent's tool call.

## See It Work

Your team has an architecture rule: the UI layer must not import from
persistence directly. You define the boundary:

```yaml
check: layer-dependency
config:
  layers:
    ui: ["src/ui/**"]
    persistence: ["src/db/**"]
  deny:
    no-persistence-import:
      description: "UI layer must not import from persistence"
      from: ui
      to: persistence
thresholds:
  fail: 0
```

A coding agent refactors a dashboard component and adds
`import { query } from '../../db/client'`. Scute catches it:

```sh
scute check layer-dependency src/ui/dashboard.ts
```

```json
{
  "check": "layer-dependency",
  "target": "src/ui/dashboard.ts",
  "status": "fail",
  "observed": 1,
  "expected": { "fail": 0 },
  "evidence": [
    {
      "location": "src/ui/dashboard.ts:3",
      "rule": "no-persistence-import",
      "found": "import { query } from '../../db/client'"
    }
  ]
}
```

The agent reads this. It knows exactly which file, which line, which rule, and
what was found. It routes the import through the domain layer and runs the
check again:

```sh
scute check layer-dependency src/ui/dashboard.ts
```

```json
{
  "check": "layer-dependency",
  "target": "src/ui/dashboard.ts",
  "status": "pass",
  "observed": 0,
  "expected": { "fail": 0 }
}
```

No human in the loop. The architecture boundary held.

### Trends, Not Snapshots

Now zoom out. Last sprint, the codebase had 3 layer violations. This sprint:

```json
{
  "check": "layer-dependency-delta",
  "target": "src/ui/",
  "status": "fail",
  "observed": 5,
  "expected": { "warn": 1, "fail": 2 },
  "baseline": { "observed": 3, "commit": "a1b2c3d" },
  "delta": 2
}
```

A snapshot says "5 violations." The trend says something changed and it's
accelerating. That's the signal that matters.

### Across the Lifecycle

The same contract works at every stage. Different checks, different thresholds,
same format:

```yaml
# During development — keep functions simple
check: cyclomatic-complexity
thresholds:
  warn: 10
  fail: 20
```

```yaml
# At PR time — don't let coverage erode
check: test-coverage-delta
thresholds:
  warn: -5
  fail: -15
```

```yaml
# In production — respect the error budget
check: error-budget
thresholds:
  warn: 20
  fail: 0
```

One definition format. One result schema. From pre-commit hook to production
monitor.

See the full [check evaluation schema](handbook/decisions/0001-check-evaluation-schema.md)
and [check definition format](handbook/decisions/0002-check-definition-format.md).

## Design Principles

Scute aspires to be a **protocol layer for fitness checks** — like OpenTelemetry
standardized observability signals, Scute standardizes fitness signals. The
check contract (structured input, structured output) is the product. Everything
else composes on top.

**Deterministic.** Checks produce facts, not suggestions. Same input, same
result, every time, on any machine.

**Agent-native.** Built for coding agents. If it makes sense for an agent, it
makes sense for a developer. Structured output means CI, editors, and machines
consume it just as naturally.

**Composable.** Pick the checks you need. Configure thresholds for your
context. Write your own. Compose them into workflows that match how your team
ships.

**No interface is the best interface.** No dashboards to maintain. No SaaS to
authenticate against. Checks are code. Results are data.

**Trends over snapshots.** A single score means little. Direction matters more
than position. Scute tracks both.

**Private by default.** Your code stays on your machines. No telemetry, no
phone-home, no cloud dependency.

For the full set of project values, see
[handbook/attributes.md](handbook/attributes.md).

## What This Is Not

- **Not a linter.** Linters check syntax and style. This checks whether your
  software is evolving in a healthy direction.
- **Not an AI code reviewer.** No LLM scoring your code. Checks are
  deterministic: same input, same result.
- **Not a platform.** No hosted service, no account, no data leaving your
  environment. A toolkit you run wherever you want.

## Who It's For

| Actor        | How they use it                                          |
| ------------ | -------------------------------------------------------- |
| Developer    | Pre-commit hooks, editor integration, local CLI          |
| Coding Agent | Structured checks as tool calls, clear pass/fail signals |
| Tech Lead    | Architecture boundary enforcement, evolution tracking    |
| SRE / Ops    | Production fitness checks, SLO verification              |
| Compliance   | Audit trails, policy-as-code, reproducible evidence      |

Checks run wherever your workflow needs them: locally, in CI, in PR reviews,
in release pipelines, in production.

## Why Open Source

Software maintainability shouldn't be gated behind a license. Same tooling
for a solo developer and a 500-person org.

Fully open source. Not "open core." Not "free tier." Open.

This project eats its own dog food: the checks we ship are the checks we run
on ourselves.

## Learn More

The [handbook](handbook/) contains the project's foundational documents:
vision, principles, roadmap, and architecture decisions.

## Status

🚧 Early development. The foundation is set — contracts, schemas, and design
principles are validated. Implementation is beginning.

We're building in the open from day one. If this resonates with how you think
about software, watch the repo, open issues, start conversations.

## License

TBD. Will be a permissive open-source license.
