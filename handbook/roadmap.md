# Roadmap

Software goes through a lifecycle: someone decides what to build, writes the
code, ships it, watches whether it works. At every step, there are questions.
Is this heading in the right direction? Is this safe to land? Is this
performing in production?

Most teams answer these questions with gut feel, manual review, or scattered
tools. Coding agents make this more urgent: they produce code faster than
humans can review it, so problems need to be caught earlier than ever.

Scute is a fitness function engine: you define the characteristics your
software must uphold, and Scute evaluates them across the lifecycle. A
**fitness function** is a deterministic check that measures how close a
system is to a desired characteristic. Pass or fail, with evidence.

## The Full Loop

The software lifecycle, mapped as a loop. At each step, a question. Scute's
job is to answer it with facts.

```text
     Prioritize ─→ Define ─→ Develop ─→ Commit ─→ Integrate
         ↑                                            |
         ╰ Learn ←── Observe ←── Release ←── Deliver ←╯
```

| Step       | Question                                | Status |
| ---------- | --------------------------------------- | ------ |
| Prioritize | What's next?                            | 🟡     |
| Define     | What characteristics matter?            | 🟡     |
| Develop    | Is this heading in the right direction? | 🟡     |
| Commit     | Is this batch ready?                    | 🟡     |
| Integrate  | Will this merge well?                   | 🟡     |
| Deliver    | Is this release safe?                   | ⬜     |
| Release    | Is it usable in production?             | ⬜     |
| Observe    | Is it satisfying users?                 | ⬜     |
| Learn      | What should we improve?                 | ⬜     |

🟡 partial — ⬜ not started

### Prioritize — What's next?

Scute is an input, not the orchestrator. It contributes signals to whatever
manages your backlog: "error budget is at 12%, new features are blocked" or
"complexity in module X crossed the ratchet." Scute doesn't own
prioritization. It populates the backlog with fitness function violations
and trend alerts.

**Today:** Dependency staleness.\
**Next:** Fitness trends over time, health overviews across the codebase,
error budget evaluation.

### Define — What characteristics matter?

One config file declares what your team cares about. Every check, at every
step, evaluates against it. If it's not defined here, it's not enforced
anywhere.

**Today:** Code-level rules (complexity thresholds, similarity tolerance,
commit format, dependency staleness).\
**Next:** Architecture constraints (dependency direction, module boundaries),
SLO targets, delivery thresholds, security policies.

### Develop — Is this heading in the right direction?

Instant feedback while you're writing code. Does this function exceed the
complexity threshold? Does this block duplicate something elsewhere? Fast,
local, no waiting for CI.

**Today:** Code complexity, code similarity.\
**Next:** Architecture rules (circular dependencies, layer violations),
security patterns, contract conformance.

### Commit — Is this batch ready?

Scute gates the commit. If it fails, the commit doesn't go through. Fast,
blocking, non-negotiable.

**Today:** Complexity, similarity, commit message format.\
**Next:** Batch size enforcement ("this commit touches too many modules"),
breaking change detection.

### Integrate — Will this merge well?

Scute runs as a CI participant. This is where checks that only make sense
with the full diff come in: cross-module impact, API contract changes,
integration fitness functions. Contract results and test signals get
evaluated against the characteristics from **Define**.

**Today:** Complexity, similarity, PR title.\
**Next:** Coverage delta, API compatibility, cross-module impact.

### Deliver — Is this release safe?

Code is on main and deployed to staging. A different class of
fitness functions kicks in: performance budgets, load test results, security
scans. All evaluated against **Define**. Scute gates the promotion to
production.

_Not started._

### Release — Is it usable in production?

Code is live, possibly behind a feature flag or canary. Scute
evaluates canary health, probe results, error rates during rollout. Are the
SLOs holding? If not, signal to roll back.

_Not started._

### Observe — Is it satisfying users?

Scute continuously evaluates production against the defined
fitness functions. Golden signals against SLO targets. When something drifts:
"Latency p99 has been above your 200ms target for 3 days."

_Not started._

### Learn — What should we improve?

Scute becomes an analyst. Which code characteristics predict
production failures? Which high-complexity modules also have high incident
rates? Trend data across all steps becomes actionable intelligence, feeding
back into **Prioritize**.

_Not started._

## What's Next

Three cross-cutting capabilities that unlock the steps above:

- **Richer config model.** Express architecture constraints, delivery
  thresholds, and SLO targets, not just code rules. This is what lets
  **Define** grow, and without it, steps beyond Integrate have nothing to
  evaluate against.
- **Broader fitness functions.** Architecture rules (circular dependencies,
  layer violations), delivery checks, security patterns. Deepens **Develop**
  through **Integrate** before reaching further.
- **Trends and ratcheting.** Track direction over time, not just snapshots.
  Set baselines, enforce "never get worse than this." Feeds **Prioritize**
  and **Learn**.

## What We Won't Build

- **Dashboards or web UI.** CLI and structured output, always.
- **AI/LLM-based code review.** Deterministic, not probabilistic.
- **A CI/CD platform.** Scute runs inside your pipeline, not around it.
