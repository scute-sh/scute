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

## Phase 2: Set Up Worktree

Create a worktree for this issue using the built-in worktree support. Name it after the issue (e.g., `fix-login-redirect`, `add-export-csv`).

**Exit:** You're working inside the new worktree directory.

---

## Phase 3: Build

Assess issue complexity to pick the right approach:

- **Substantial feature or complex change** → invoke `/feature-development`. Each slice is a shippable PR — after completing a slice, go to Phase 4 and 5 before starting the next slice. Each PR must not break existing behavior. Only the final PR gets `Closes #<number>`.
- **Small/focused fix** (bug fix, config change, small refactor) → use `/tdd` or the appropriate testing skill directly
- **Unclear** → ask the user

The build phase belongs to whatever skill you invoke. Follow its process fully.

**Exit:** All changes committed, all tests pass.

---

## Phase 4: Open PR

1. Push the branch
2. Create the PR:
   - Title: short, under 70 characters
   - Body: use the repo's PR template if one exists, otherwise write a concise description. Include `Closes #<number>` on the final PR to auto-close the issue. For intermediate PRs, reference the issue without closing it (`Part of #<number>`).
3. Report the PR URL to the user

**Exit:** PR is open and linked to the issue.

---

## Phase 5: Watch CI and Iterate

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
- PR URL
- Summary of what was done
- Any decisions made along the way that the user should know about

Do not clean up the worktree — the user may want to keep it for follow-up work.
