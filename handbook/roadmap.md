# Roadmap

## Phase 1: The Engine

- Check definition parser (YAML)
- Check execution and result collection
- Structured JSON output
- CLI (`scute check <check>`)
- 3-5 first-party checks: cyclomatic complexity, circular dependencies,
  layer dependency, dependency freshness, test coverage
- MCP server as a first-class interface from day one

## Phase 2: The Feedback Loop

- Agent workflow integration: agent generates code, scute checks, failures
  fed back as structured context, agent self-corrects
- Adapter protocol for wrapping external tools as Scute checks
- Scope resolution (changed-files, staged files, PR diff, all files)

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
