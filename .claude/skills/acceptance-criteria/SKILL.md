---
name: acceptance-criteria
description: Defines acceptance criteria from requirements through persona analysis and refinement. Use when clarifying requirements, defining what done looks like, or writing acceptance criteria.
---

# Acceptance Criteria

STARTER_CHARACTER = 🎯

Define what success looks like before thinking about code. Stay in the problem space — if you're naming functions, data structures, or config keys, you've left it.

## Process

1. **Restate the requirement** in your own words

2. **Identify personas** — who will actually interact with this feature, directly or indirectly? Don't settle for the obvious ones. Think about:
   - Who triggers it? (human, agent, CI, cron?)
   - Who consumes the output?
   - Who configures it?
   - Who debugs it when it breaks?

   Present the personas to the user before continuing. You are likely wrong or incomplete — get confirmation.

3. **Walk through each persona's real workflow** — not the idealized flow, the actual one:
   - What are they doing *right before* they encounter this feature?
   - What do they see/do step by step?
   - What tells them it worked?
   - What tells them it didn't?

   This is where weak personas get exposed. If you can't describe a concrete, realistic workflow, the persona is too vague.

4. **From those workflows, identify:**
   - **Inputs**: What data/events trigger this?
   - **Outputs**: What should change? What should users see?
   - **Boundaries**: What's explicitly out of scope?
   - **Unknowns**: What needs clarification?

5. If unknowns exist, ask the user — don't guess on important decisions

6. **Write criteria** to `playground/{feature-name}-criteria.md`:

   ```
   # [Feature Name] Acceptance Criteria

   ## Personas
   - [Persona]: [one-line description of their relationship to this feature]

   ## Must Have
   - [ ] Criterion 1 (specific, testable)
   - [ ] Criterion 2

   ## Out of Scope
   - Item explicitly excluded
   ```

Use a short, hyphenated feature name (e.g., `user-auth`, `invoice-export`) consistently across all playground files.

## Gate

_Invoke_ `/refinement-loop` on the criteria file with these checks:

- Every persona has at least one criterion that specifically serves them?
- Specific enough to test? (not "works well" but "returns X when given Y")
- Complete? Walk through each persona's workflow — any gaps?
- Bounded? Clear what's NOT included?

Do not consider this done until every criterion passes every check.
