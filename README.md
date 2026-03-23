<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/scute-logo_white.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/scute-logo_blue.svg">
  <img alt="Scute" src="assets/scute-logo_blue.svg" width="200">
</picture>

**Define the boundaries. Let your code evolve freely within them.**

[![CI](https://github.com/scute-sh/scute/actions/workflows/ci-scute.yml/badge.svg)](https://github.com/scute-sh/scute/actions/workflows/ci-scute.yml)
[![crates.io](https://img.shields.io/crates/v/scute)](https://crates.io/crates/scute)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

</div>

An open-source toolkit for deterministic fitness checks, guardrails, and
Harness Engineering across your entire software delivery lifecycle.
Built for developers and coding agents alike.

## The Slow Rot

We check whether code compiles. We check whether tests pass. We rarely check
whether the software itself is heading in the right direction. Are architecture
boundaries holding? Is complexity creeping? Are dependencies drifting? Are we
honoring our SLOs?

Most teams don't lack standards. They lack enforcement that's automated,
measurable, and doesn't rot alongside the codebase. And coding agents make this
even more urgent. They produce code faster than humans can review it,
without worrying whether the codebase is getting better or worse.

Scute gives your codebase a protective shell that evolves with it,
so your product keeps heading in the right direction.

## What It Looks Like

You just finished a refactor. Before committing, you want to know if any
function got too complex:

```sh
scute check code-complexity src/
```

```json
{
  "check": "code-complexity",
  "summary": { "evaluated": 14, "failed": 1 },
  "findings": [
    {
      "target": "src/engine.rs:87:resolve_config",
      "status": "fail",
      "measurement": {
        "observed": 12,
        "thresholds": { "warn": 5, "fail": 10 }
      },
      "evidence": [
        {
          "rule": "nesting",
          "location": "src/engine.rs:92",
          "found": "'if' nested 2 levels: 'for > match > if' (+3)"
        }
      ]
    }
  ]
}
```

One function failed. Scored 12 against a threshold of 10, because of deep
nesting at a specific line. Not a vague "this file is complex." Exact
location, exact rule, exact score. Fix it, rerun, pass.

Yes, you could stitch this together yourself: a complexity linter here, a
dependency checker there, a service level objective rule somewhere else.
Different tools, different configs, different output formats.

Scute does it for you. It builds on top of your existing ecosystem and wraps
everything in one structured contract, for every check. Your terminal,
your agent, and your CI all consume it identically.

## What It Checks

| Check                                                         | What it catches                        | Supports             |
| ------------------------------------------------------------- | -------------------------------------- | -------------------- |
| [`code-complexity`](docs/checks/code-complexity.md)           | Functions that are hard to understand  | Rust, JS, TS         |
| [`code-similarity`](docs/checks/code-similarity.md)           | Copy-paste and structural duplication  | Rust, JS, TS         |
| [`commit-message`](docs/checks/commit-message.md)             | Sloppy or non-standard commit messages | Conventional commits |
| [`dependency-freshness`](docs/checks/dependency-freshness.md) | Dependencies drifting behind           | Cargo, npm, pnpm     |

More checks and ecosystems are on the [roadmap](handbook/roadmap.md).

## Install

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
cargo install scute        # from source
cargo binstall scute       # pre-built binary
```

## Configure

Sensible defaults out of the box. Override what you need in `.scute.yml`:

```yaml
checks:
  code-complexity:
    thresholds:
      warn: 8
      fail: 15
  commit-message:
    types: [feat, fix, docs, refactor, test, chore, hotfix]
  dependency-freshness:
    level: minor
    thresholds:
      warn: 3
      fail: 6
```

## Guide Your Agents

Scute ships with both a CLI and an MCP server, so it works with any coding
agent. When a check fails, the agent gets structured evidence (exact locations,
exact rules) and can fix the issue before you ever review it.

### Tell your agent when to check

Add instructions to your agent config (`CLAUDE.md`, cursor rules, etc.):

```markdown
## Scute Checks

Run proactively, not only when asked:

| Check              | When to run                           |
| ------------------ | ------------------------------------- |
| code-complexity    | After changing or adding a function   |
| code-similarity    | After changing or adding a function   |
| commit-message     | Before committing                     |
| dependency-freshness | After adding or updating a dependency |

When a check returns warnings or failures, read the evidence, fix the issue,
and rerun until it passes. Don't defer warnings you've just introduced.
```

The agent runs a check, it fails, the evidence says exactly what's wrong and
where. The agent fixes it, reruns, passes. No human in the loop in most cases.

### Connect via MCP

If your agent supports [MCP](https://modelcontextprotocol.io), register
Scute's server. The agent gets check tools it can call directly, plus built-in
guidance on how to interpret results.

Add to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "scute": { "type": "stdio", "command": "scute", "args": ["mcp"] }
  }
}
```

### Or use the CLI

Agents that don't support MCP can run Scute as shell commands. The output
is the same structured JSON either way.

```sh
scute check code-complexity src/
scute check commit-message "feat: add caching layer"
```

## Guide Your Commits

Wire Scute into git hooks so every commit gets checked automatically.

```sh
# .git/hooks/pre-commit
#!/bin/sh
set -e
git diff --cached --diff-filter=d --name-only | scute check code-complexity
git diff --cached --diff-filter=d --name-only | scute check code-similarity
```

```sh
# .git/hooks/commit-msg
#!/bin/sh
set -e
scute check commit-message "$(cat "$1")"
```

Works the same whether you're committing or your coding agent is.

## Guide Your Pull Requests

The last gate before merge. You decide which checks run at which stage. The
contract stays the same: one definition for the entire codebase.

Exit codes are designed for automation: `0` pass, `1` fail, `2` error.

```yaml
# GitHub Actions — runs on pull requests
- run: git diff --diff-filter=d --name-only origin/main...HEAD | scute check code-complexity
- run: git diff --diff-filter=d --name-only origin/main...HEAD | scute check code-similarity
- run: scute check commit-message "${{ github.event.pull_request.title }}"
```

## Guide Your Codebase

Schedule checks that don't belong in a PR but matter over time.

```yaml
# GitHub Actions — runs on a schedule
on:
  schedule:
    - cron: "0 9 * * 1" # every Monday at 9am

steps:
  - run: scute check dependency-freshness
```

Dependencies drift whether you're shipping or not. Catch it before it
compounds.

## What Drives Scute

**Deterministic.** Checks produce facts, not suggestions. Same input, same
result, every time, on any machine.

**Agent-native.** If it makes sense for an agent, it makes sense for a
developer. Structured output means CI, editors, and machines consume it just
as naturally.

**Composable.** Pick the checks you need. Configure thresholds for your
context. Compose them into workflows that match how your team ships.

**Private by default.** Your code stays on your machines. No telemetry, no
phone-home, no cloud dependency.

**Trends over snapshots.** A single score means little. Direction matters more
than position.

For the full set of project values, see the [project handbook](handbook/).

## Status

Early and moving fast. Four checks, CLI, MCP server, and structured output are
shipping today.

**Next:** code complexity for JS, more package managers (Deno, Bun, Yarn),
trend tracking (delta-from-baseline, direction over time), new checks (circular
dependencies, layer violations).

## License

Fully open source. Not "open core." Not "free tier." Open.

Apache 2.0. See [LICENSE](LICENSE).
