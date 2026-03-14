# Scute

**Define the boundaries. Let your code evolve freely within them.**

An open-source toolkit for deterministic fitness checks, guardrails, and
Harness Engineering across your entire software delivery lifecycle.
Built for developers and coding agents alike.

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

A coding agent adds a utility function to `src/utils/format.rs`. Scute checks
for structural duplication across the project, focusing on the new file:

```sh
scute check code-similarity src/utils/format.rs
```

```json
{
  "check": "code-similarity",
  "summary": {
    "evaluated": 12,
    "passed": 11,
    "warned": 0,
    "failed": 1,
    "errored": 0
  },
  "findings": [
    {
      "target": "src/utils/format.rs:14",
      "status": "fail",
      "measurement": {
        "observed": 128,
        "thresholds": { "warn": 70, "fail": 100 }
      },
      "evidence": [
        {
          "location": "src/utils/format.rs:14-38",
          "found": "128 duplicated tokens, e.g. `fn format_timestamp(ts: i64) -> String {`"
        },
        {
          "location": "src/helpers/time.rs:7-31",
          "found": "128 duplicated tokens, e.g. `fn format_time(ts: i64) -> String {`"
        }
      ]
    }
  ]
}
```

The agent reads this. 128 tokens of structural duplication between
`src/utils/format.rs:14-38` and `src/helpers/time.rs:7-31`. Same function,
different location. It consolidates: removes the duplicate, reuses the existing
one, and reruns the check.

```sh
scute check code-similarity src/utils/format.rs
```

```json
{
  "check": "code-similarity",
  "summary": {
    "evaluated": 12,
    "passed": 12,
    "warned": 0,
    "failed": 0,
    "errored": 0
  },
  "findings": []
}
```

Pass. No human in the loop. The duplication was caught before it reached a PR.

Every check produces the same structured JSON: what was checked, what was
observed, what the thresholds are, and exactly what evidence triggered the
result. Agents and CI consume it the same way.

## Available Checks

| Check                                                         | What it catches                 | Scope                        |
| ------------------------------------------------------------- | ------------------------------- | ---------------------------- |
| [`code-complexity`](docs/checks/code-complexity.md)            | Cognitive complexity per function | Rust                         |
| [`code-similarity`](docs/checks/code-similarity.md)           | Structural code duplication     | Rust, JavaScript, TypeScript |
| [`commit-message`](docs/checks/commit-message.md)             | Conventional Commits violations | Any project                  |
| [`dependency-freshness`](docs/checks/dependency-freshness.md) | Outdated dependencies           | Cargo, npm, pnpm             |

## Quickstart

### Install

macOS / Linux:

```sh
curl -fsSL scute.sh/install | sh
```

Windows:

```powershell
irm scute.sh/install.ps1 | iex
```

Or via [Homebrew](https://brew.sh) / [Cargo](https://doc.rust-lang.org/cargo/):

```sh
brew install scute-sh/tap/scute
cargo install scute
cargo binstall scute
```

### Run a check

```sh
scute check commit-message "feat: add login"
```

```json
{
  "check": "commit-message",
  "summary": {
    "evaluated": 1,
    "passed": 1,
    "warned": 0,
    "failed": 0,
    "errored": 0
  },
  "findings": []
}
```

Pass. No config needed, sensible defaults out of the box.

Now try your team's custom commit type:

```sh
scute check commit-message "scute: reporting for duty 🐢"
```

```json
{
  "check": "commit-message",
  "summary": {
    "evaluated": 1,
    "passed": 0,
    "warned": 0,
    "failed": 1,
    "errored": 0
  },
  "findings": [
    {
      "target": "scute: reporting for duty 🐢",
      "status": "fail",
      "measurement": { "observed": 1, "thresholds": { "fail": 0 } },
      "evidence": [
        {
          "rule": "unknown-type",
          "found": "scute",
          "expected": [
            "feat",
            "fix",
            "docs",
            "style",
            "refactor",
            "perf",
            "test",
            "build",
            "ci",
            "chore",
            "revert"
          ]
        }
      ]
    }
  ]
}
```

`scute` isn't a standard Conventional Commits type. The `evidence` tells you
exactly what went wrong: the type was unknown, and here are the ones it accepts.

Drop a `.scute.yml` in your project root to make it yours:

```yaml
checks:
  commit-message:
    types: [feat, fix, docs, refactor, test, chore, scute]
```

Run it again:

```sh
scute check commit-message "scute: reporting for duty 🐢"
```

```json
{
  "check": "commit-message",
  "summary": {
    "evaluated": 1,
    "passed": 1,
    "warned": 0,
    "failed": 0,
    "errored": 0
  },
  "findings": []
}
```

Pass. The config is optional, but it's there when you need it.

## Agent Integration

Scute ships an MCP server so coding agents can run checks as tool calls.

### 1. Register the MCP server

Add this to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "scute": {
      "type": "stdio",
      "command": "scute",
      "args": ["mcp"]
    }
  }
}
```

### 2. Tell your agent to use it

Add instructions to your project's agent config (e.g. `CLAUDE.md`, cursor
rules, etc.) telling the agent to use Scute checks proactively. Here's what
Scute's own config looks like:

```markdown
# MCP Tools Are Part of the Workflow

Scute's MCP server has check tools. Use them proactively:

- `check_commit_message` — before making a commit
- `check_code_complexity` — after changing a function or implementing a new one
- `check_code_similarity` — after changing a function or implementing a new one
- `check_dependency_freshness` — after adding or updating a dependency
```

### 3. The agent self-corrects

The MCP server's built-in instructions already tell agents to use evidence
for self-correction. When a check fails, the agent gets structured evidence
(files, locations, what was found, what was expected) and can fix the issue
before asking for review.

## The Bigger Picture

The contract is the product. Every check, whether it runs in your editor, in
CI, or in production, speaks the same language: structured input, structured
output, deterministic results. One format from pre-commit hook to production
monitor.

That's where we're headed. The checks available today are just the beginning.
The same contract will support checks like:

```yaml
checks:
  # Detect hidden dependencies from change patterns
  change-coupling:
    window: 90d
    min-commits: 10
    thresholds:
      warn: 30    # coupling degree %
      fail: 60

  # Don't let coverage drift, ...
  test-coverage-drift:
    thresholds:
      warn: -5
      fail: -15

  # Catch latency regressions
  service-latency:
    percentile: p99
    window: 7d
    thresholds:
      warn: 150   # ms
      fail: 300
```

Think of it like OpenTelemetry for fitness checks. OTel standardized
observability signals; Scute standardizes fitness signals. The check contract
is the protocol layer. Everything else composes on top.

## Design Principles

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

**What's done:**

- 4 checks: `code-complexity`, `code-similarity`, `commit-message`, `dependency-freshness`
- CLI with structured JSON output
- MCP server for coding agent integration
- Agent self-correction workflow (fail → read evidence → fix → pass)

**What's next:**

- More checks (circular dependencies, layer dependency)
- Broader ecosystem support (deno, node, java, etc.)
- Trend tracking (delta-from-baseline, direction over time)

We're building in the open from day one. If this resonates with how you think
about software, watch the repo, open issues, start conversations.

## License

Apache 2.0. See [LICENSE](LICENSE).
