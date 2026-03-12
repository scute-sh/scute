---
name: issue-workflow
description: End-to-end GitHub issue lifecycle: pick up an issue, create a worktree, build the fix or feature, open a PR, watch CI, and iterate until green. Use when picking up, fixing, or implementing a GitHub issue or ticket.
---

# Issue Workflow

STARTER_CHARACTER = 🎫

Full lifecycle for working on a GitHub issue. Five phases, each with a clear exit condition.

## Phase 1: Understand the Issue

1. Read the issue and its comments
2. Restate the problem and the expected outcome in your own words
3. Identify blindspots — if something is ambiguous or underspecified, ask the user before proceeding. Do not assume.
4. Assign the issue to the user

**Exit:** You can explain what needs to change and why, with no open questions.

---

## Phase 2: Plan and Build

Assess issue complexity to pick the right approach:

- **Substantial feature or complex change** → invoke `/feature-development`
- **Small/focused fix** (bug fix, config change, small refactor) → use `/tdd` or the appropriate testing skill directly
- **Unclear** → ask the user

The build phase belongs to whatever skill you invoke. Follow its process fully.

### Acceptance criteria belong to the issue

When the build process produces acceptance criteria, update the issue description to include them. Criteria are the shared contract — they belong with the issue, not in a local file. Other planning artifacts (scout report, sketch) are working files that stay local in the worktree.

### Multi-PR issues

For substantial work, each slice is a shippable PR. The cycle per slice:

1. Create a fresh worktree from latest main
2. Build the slice
3. Go to Phase 3 and 4 (PR → CI green)
4. Worktree is disposable after merge

Each PR must not break existing behavior. If local planning files are lost between worktrees, re-derive slices from the criteria on the issue.

### Single-PR issues

For small/focused fixes: create one worktree, build, then proceed to Phase 3.

**Exit:** All changes committed, all tests pass.

---

## Phase 3: Open PR

1. Push the branch
2. Create the PR:
   - Title: short, under 70 characters
   - Body: use the repo's PR template if one exists, otherwise write a concise description. Include `Closes #<number>` on the final (or only) PR to auto-close the issue. For intermediate PRs, reference the issue without closing it (`Part of #<number>`).
3. Report the PR URL to the user

**Exit:** PR is open and linked to the issue.

---

## Phase 4: Watch CI and Iterate

1. Watch the PR's CI checks until they complete
2. If all checks pass → done
3. If checks fail:
   - Read the failure logs
   - Fix the issue in the worktree
   - Commit and push
   - Watch again
4. After 3 failed fix attempts, stop and ask the user for guidance instead of continuing to guess.

**Exit:** All CI checks green.

---

## When Done

Report to the user:
- PR URL(s)
- Summary of what was done
- Any decisions made along the way that the user should know about

Do not clean up the worktree — the user may want to keep it for follow-up work.
