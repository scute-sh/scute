---
name: issue-workflow
description: End-to-end GitHub issue lifecycle: pick up an issue, create a worktree, build the fix or feature, open a PR, watch CI, and iterate until green. Use when picking up, fixing, or implementing a GitHub issue or ticket.
---

# Issue Workflow

STARTER_CHARACTER = 🎫

Full lifecycle for working on a GitHub issue.

## 1. Understand the Issue

1. Read the issue and its comments
2. Restate the problem and the expected outcome in your own words
3. Identify blindspots — if something is ambiguous or underspecified, ask the user before proceeding. Do not assume.
4. Assign the issue to the user

You should be able to explain what needs to change and why, with no open questions.

## 2. Define Acceptance Criteria

Do not explore or read the codebase yet. Criteria describe the user's need, not the implementation. The issue itself has all the context you need.

_Invoke_ `/acceptance-criteria` to define what done looks like. Update the issue description to include the criteria — they are the shared contract and belong with the issue, not in a local file.

## 3. Prepare the Work

Assess issue complexity:

- **Substantial feature or complex change** → _invoke_ `/scout-codebase` (if modifying existing code), then _invoke_ `/sketch-slices` to decompose into scenarios
- **Small/focused fix** → skip to the next step

Enter a worktree for this issue using `EnterWorktree` — do not create one manually with git commands. Name it after the issue (e.g., `fix-login-redirect`, `add-export-csv`).

## 4. Build and Ship

For each scenario (or the single fix), complete **all** steps before starting the next:

1. Build it — _invoke_ `/atdd` for substantial work, `/tdd` or the appropriate testing skill for small fixes
2. **Ship it** — push the branch and open a PR
   - Title: short, under 70 characters
   - Body: use the repo's PR template if one exists, otherwise write a concise description. Reference the issue with `Part of #<number>`.
3. Watch CI checks until they complete
   - If checks fail: read the failure logs, fix, commit, push, watch again
   - After 3 failed fix attempts, stop and ask the user for guidance
4. Report the PR URL to the user and **stop** — wait for the PR to be reviewed and merged before continuing
5. Once the PR is merged, reset the worktree to latest main: `git reset --hard origin/main`

Each PR must not break existing behavior.

## 5. Harden and Close

Once all scenarios are complete:

1. _Invoke_ `/harden` on all files touched
2. _Invoke_ `/verify-criteria` against the criteria on the issue
3. Push, open a final PR with `Closes #<number>` in the body
4. Watch CI until green

## When Done

Report to the user:
- PR URL(s)
- Summary of what was done
- Any decisions made along the way that the user should know about

Do not clean up the worktree — the user may want to keep it for follow-up work.
