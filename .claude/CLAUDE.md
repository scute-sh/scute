- See @README for the overview of what Scute is about.
- See @handbook/vision.md @handbook/attributes.md @handbook/roadmap.md for project context, goals, and direction.
- Don't be lazy. Don't cut corners. Think before applying a pattern.
- Before implementing a non-trivial solution, present 2-3 alternatives with trade-offs. We decide together, then build.

# Workflow

## MCP Tools Are Part of the Workflow

Don't treat CLAUDE.md checklists as exhaustive. Before any action, think about which MCP tools are relevant to what you're about to do. Scute's own MCP server has check tools (e.g., `check_commit_message`, `check_dependency_freshness`). Use them. If you're writing a commit message, validate it with the tool you literally helped build.

## 🛑 When course-corrected

**Trigger:** The user corrects, reminds, or redirects you. This includes explicit corrections AND gentle nudges like "did you forget...?"

**Action (do this BEFORE fixing the problem):**

1. Acknowledge what went wrong and why you didn't catch it yourself
2. Open @handbook/pain-points.md and track it:
   - Already documented → increase the counter (×N)
   - Not documented → write it down
   - Focus on the core problem, not the details
   - Group in logical categories

## Explore by testing, not by poking

When you need to understand how something behaves (an API, an edge case, a library), write a test and run it. Don't poke around with throwaway scripts or read source to guess. The test harness gives you a faster feedback loop and the answer sticks around as documentation.

## Before committing

- Run all three, in this order: `cargo fmt`, `cargo clippy`, `cargo test`. All must pass. Don't treat "tests pass" as "ready to commit."
- Take a step back, and think. Don't blindly follow the workflow.

# Testing

## Test naming convention

Test names are living documentation. Just by reading a name, you should understand exactly what happens and why it failed.

**Rules:**

1. **Active voice.** The implicit subject is the system under test. Use active verbs.
2. **Self-documenting.** Every noun must be grounded. If it raises "of what?" or "where?", the name is incomplete.
3. **Context before outcome** when context is needed. When no context, verb+object is fine.

```
// Good — active verbs, complete story
rejects_empty_description
accepts_scope_in_parentheses
strips_git_comment_lines
no_outdated_deps_returns_pass_with_all_fields
passing_check_exits_with_code_0
outdated_report_excludes_transitive_dependencies

// Bad — passive voice
empty_description_is_a_violation
scope_in_parentheses_is_accepted
git_comment_lines_are_stripped

// Bad — incomplete, raises questions
dev_dependencies_are_included          // included WHERE?
reports_current_version                // of WHAT?
```

# Tools

- Do not use `git -C` commands, instead `cd` at the root and use regular `git` commands.
- Cargo workspace root is `crates/`, not the repo root. Run cargo commands from there.

# Voice & Tone

The tone is **informal**, like friends working together in a startup. Not a big corpo, not a government press release. Just direct & respectful communication between devs who've seen the trenches and focus on what really matters.

This applies to everything: the product UI, marketing materials, social media, emails, docs, readmes, commit messages, ... Keep it human. Yes, even commit messages. Don't default to dry, corporate autopilot.

## Drop the em dashes

In structural texts (e.g. lists/enumerations), that's ok. But not in regular prose. Humans don't use it in everyday communication, so you should not either.

# Last Words

We're here to have fun! 💃

Building projects is an awesome creative process, so let's have a blast!
