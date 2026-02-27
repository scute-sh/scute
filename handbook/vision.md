# Vision

## The Bet

Coding agents produce code faster than humans can review it. This amplifies
the pressure on developers in two ways: guiding the development process toward
the right outcomes (quality assurance) and verifying that what's produced actually
meets the bar (quality control).

When code is produced at machine speed, catching problems at the PR stage is
already too late. Verification must shift left into the editor, the terminal,
the agent's own feedback loop, so that fitness checks run _before_ code is
proposed, not after. Teams don't lack standards; they lack enforcement that's
automated, measurable, and doesn't rot.

Existing tools (SonarQube, CodeScene, Code Climate) are dashboard-centric, built
for humans clicking through a UI. The industry needs deterministic verification
designed for agents and terminals first, humans second.

Scute is the **deterministic fitness check layer** for software delivery. It sits
between code generation (by agents or humans) and code acceptance (by CI, PR gates,
or humans). It answers one question: does this code meet the criteria you defined?

## Strategic Position

Scute is not an orchestrator, not a platform, not a linter.

It aspires to be a **protocol layer for fitness checks**. Like OpenTelemetry for
observability, but for code fitness. The check contract (structured input,
structured output) is the product. Everything else composes on top.

## Schema Validation

The check result schema ([ADR-0001](decisions/0001-check-result-schema.md)) and
check definition format ([ADR-0002](decisions/0002-check-definition-format.md))
were validated against diverse fitness function types:

| Check Type                       | observed                                | Outcome                   |
| -------------------------------- | --------------------------------------- | ------------------------- |
| Cyclomatic complexity            | numeric metric                          | Clean fit                 |
| Layer violations                 | violation count + evidence              | Clean fit                 |
| Circular dependencies            | cycle count + graph evidence            | Clean fit                 |
| Naming conventions               | violation count + per-instance evidence | Clean fit                 |
| Dependency freshness             | major versions behind                   | Clean fit                 |
| Test coverage (absolute)         | percentage                              | Clean fit                 |
| Test coverage (delta)            | percentage change                       | Separate check, clean fit |
| Response time p99                | milliseconds                            | Clean fit                 |
| Error budget / SLO               | remaining budget %                      | Clean fit                 |
| API contract drift               | violation count + structured evidence   | Clean fit                 |
| Service communication governance | violation count                         | Clean fit                 |
| Known vulnerabilities            | count above severity threshold          | Clean fit                 |

## Intellectual Foundation

- **Building Evolutionary Architectures** (Ford, Parsons, Kua, Sadalage):
  fitness functions as automated governance. The taxonomy of scope, cadence,
  result, invocation, proactivity, and coverage directly informed the schema
  design.
- **Accelerate / DORA**: leading indicators that predict delivery performance.
- **The Practice of Cloud System Administration**: operational maturity and
  systematic thinking.
- **SRE (Google)**: error budgets, SLOs, and operations as a software problem.
- **Chaos Engineering**: verifying resilience through deliberate experimentation.
- **Escaping the Build Trap**: shipping outcomes, not output.
- **Toyota Kata**: continuous improvement through small, deliberate experiments.
- **Atomic Habits**: small, compounding changes that reshape systems over time.
- **Genetic algorithms**: fitness functions evaluate how close a solution is to
  ideal. Coding agents are the mutation; Scute is the selection pressure.

The research that preceded this project identified three market gaps:

1. The "missing middle" between probabilistic generation and rigid legacy
   validation.
2. Privacy-first verification that keeps code local.
3. Agent-native CLI output (structured, semantically dense, token-efficient).
