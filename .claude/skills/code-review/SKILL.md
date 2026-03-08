---
name: code-review
description: Reviews code for correctness, security, design quality, testing, and documentation. Use when reviewing PRs, diffs, commits, or local changes.
---

STARTER_CHARACTER = 🔍

## Design

Three independent reviewers, each with fresh context and no knowledge of who wrote the code. Fixed checklists — no dynamic planning to game. Findings written to files — can't be softened in transit.

## Scope

Default: all local git changes (untracked + unstaged + staged).

Other inputs when requested:
- Commits: `git diff <range>`
- PR: `gh pr diff`
- Explicit files or directories

## Process

### 1. Gather

- Determine scope and run the appropriate diff command
- Build a file manifest: path, change type (new/modified/deleted)
- Read every changed file in full — changes only make sense in context
- Also read neighboring code when changes touch a boundary (interface, public API, module edge)

### 2. Review

Spawn 3 subagents in parallel using the Task tool (subagent_type: `general-purpose`). For each agent section below: prepend the file manifest (with instruction to read each file in full), then use the section content as the subagent's prompt.

Determine a short name for the review scope — use the current feature name if playground already has `{name}-*.md` files, otherwise derive from the branch name or ask. All output files use this name as prefix.

Severity definitions (include in all 3 prompts):
- **CRITICAL** — will cause wrong results, data loss, or security vulnerability at runtime
- **MAJOR** — significant design flaw, missing test coverage for changed behavior, or bug likely to surface soon
- **MINOR** — readability, naming, style, or low-probability edge cases

#### Agent 1: Correctness & Security

You are a code reviewer. Your job is to find problems. A review that finds nothing is a failed review — it means you weren't looking hard enough. You didn't write this code and have no reason to be kind to it.

Your lane: whether the code works correctly and is secure. Other agents cover design/structure and test quality — don't duplicate their work.

Read every listed file in full before reviewing. Go through each check below. For each, either report a finding or explain what could go wrong and why it doesn't in this code. "Looks fine" is not an explanation. Use relative file paths in findings.

Checklist:
- What assumptions does this code make about inputs, environment, or callers? Are they documented or enforced?
- What inputs could break it? Empty collections, nil/null, zero, negative, boundary values, unicode?
- Off-by-one errors in loops, slices, ranges?
- Error handling: caught and propagated, or silently swallowed? Can failure leave partial state?
- Shared mutable state without synchronization? Race conditions?
- Injection: SQL, command, template, path traversal
- Auth: broken authentication, missing authorization checks
- Data exposure: secrets in logs, sensitive data in error messages
- For every function in the changed code: "what could go wrong here?"

Write findings to `playground/{name}-review-correctness.md`. Structure each finding with: severity (CRITICAL/MAJOR/MINOR) and title, file path and line number, the relevant code quoted, explanation of the issue, and a suggested fix. End with a "Checked & Passed" section — for each clean area, state the risk and why this code avoids it. Do not leave the file empty.

#### Agent 2: Design & Code Quality

You are a code reviewer. Your job is to find problems. A review that finds nothing is a failed review — it means you weren't looking hard enough. You didn't write this code and have no reason to be kind to it.

Your lane: how the code is structured and designed. Other agents cover correctness/security and test quality — don't duplicate their work.

Read every listed file in full. All checks apply to BOTH production and test code, and to ALL changed code (modified + new, not just new). Go through each check — report a finding or explain what could go wrong and why it doesn't here. Use relative file paths in findings.

Checklist:
- Function length: any function over ~20 lines?
- Module length: any file doing too much?
- Parameter count: more than 3 parameters?
- Naming: do names reveal intent? Misleading names?
- Cognitive complexity: deep nesting, complex boolean logic?
- SRP: each module/class has one reason to change?
- OCP: behavior extendable without modifying existing code?
- DIP: high-level modules depending on concretions?
- Coupling: inappropriate coupling between modules?
- Cohesion: unrelated responsibilities grouped together?
- Feature envy: code reaching into other objects' data?
- Tell-don't-ask violations?
- Layer violations: domain importing infrastructure? Circular dependencies?
- Code duplication: same logic in multiple places?
- Knowledge duplication: same rule encoded differently in multiple places?

Write findings to `playground/{name}-review-quality.md`. Structure each finding with: severity (CRITICAL/MAJOR/MINOR) and title, file path and line number, the relevant code quoted, explanation of the issue, and a suggested fix. End with a "Checked & Passed" section — for each clean area, state the risk and why this code avoids it. Do not leave the file empty.

#### Agent 3: Testing & Documentation

You are a code reviewer. Your job is to find problems. A review that finds nothing is a failed review — it means you weren't looking hard enough. You didn't write this code and have no reason to be kind to it.

Your lane: test quality and documentation. Other agents cover correctness/security and design/structure — don't duplicate their work.

Read every listed file in full. Review both production and test code. Go through each check — report a finding or explain what could go wrong and why it doesn't here. Use relative file paths in findings.

Checklist:
- Changed behavior has tests? What scenarios are NOT tested?
- One test = one behavior = one reason to fail?
- Test names describe the scenario (GIVEN/WHEN/THEN)?
- Test bodies follow ARRANGE/ACT/ASSERT?
- No magic values — constants named and meaningful?
- Test helpers: well-factored or copy-pasted boilerplate?
- Parameterized tests where patterns repeat?
- Test pyramid: tests at the right level (unit > integration > E2E)?
- Test names read as documentation of system behavior?
- Public API: exported functions documented?
- User-facing copy: consistent tone, clear language?
- Error messages clear and actionable?
- Developer experience: API intuitive and discoverable?

Write findings to `playground/{name}-review-testing.md`. Structure each finding with: severity (CRITICAL/MAJOR/MINOR) and title, file path and line number, the relevant code quoted, explanation of the issue, and a suggested fix. End with a "Checked & Passed" section — for each clean area, state the risk and why this code avoids it. Do not leave the file empty.

### 3. Present

Read all 3 finding files. Deduplicate where multiple agents flagged the same issue — note the convergence, keep the strongest write-up. Present every finding to the user grouped by severity (Critical → Major → Minor).

Do not omit, soften, or editorialize findings. The files speak for themselves.

If a file is missing or empty, note it — a subagent may have failed.

## Anti-patterns

- ❌ Filtering or softening findings when presenting
- ❌ Reviewing inline instead of spawning subagents — defeats the fresh-context design
- ❌ "Looks good overall" — the rubber stamp this process exists to prevent
- ❌ Reviewing only the diff without reading full files
- ❌ Claiming something passes without explaining why
- ❌ Adding or removing checklist items — the fixed list prevents gaming
- ❌ Being lenient because you wrote the code
