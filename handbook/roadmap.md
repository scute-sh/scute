# Roadmap

## Phase 1: Foundation ✅

- Check engine, CLI, structured JSON output
- MCP server as a first-class interface
- Agent workflow integration (fail → evidence → self-correct → pass)
- 4 checks: code-complexity, code-similarity, commit-message, dependency-freshness

## Phase 2: Expand

- More checks: circular dependencies, layer dependency, change coupling
- Broader ecosystem: deno/pnpm support for existing checks, npm
  dependency-freshness
- Scope resolution (standardized approach for changed-files, staged, PR diff,
  all)

## Phase 3: Trends

- Local trend store (check results indexed by commit/timestamp)
- Delta-from-baseline as a first-class query
- `scute trend <check> --since 2w` to track direction, not just position
- Delta checks powered by stored baselines

## Phase 4: Broader Lifecycle

- Pre-code checks (dependency policies, architecture constraints)
- PR-level gates and change impact analysis
- Release checks (API contract drift, changelog completeness)
- Production checks (SLO compliance, error budgets)

## What We Won't Build

- Dashboards or web UI
- AI/LLM-based code review
- A CI platform or execution engine
- A plugin system before the core contract is stable
- Formal methods integration
- AST-based code transformation (use GritQL, ast-grep; wrap, don't compete)
- API contract testing (use Pact, Specmatic; wrap, don't compete)
