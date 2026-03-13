---
name: atdd
description: "Drives implementation outside-in: starts from an acceptance test for user-visible behavior, then works inward layer by layer, stubbing what doesn't exist yet. Use when building a feature across collaborating components to let design emerge from usage rather than speculation."
---

# ATDD — Acceptance Test-Driven Development

STARTER_CHARACTER = 🧅

Build from the outside in. Start where the user is, work inward until the need is met.

## Test Strategy

Choose the right testing skill based on what you're building:

- **Behavior-heavy** (user flows, rules) → `/bdd-with-approvals`
- **Algorithm/logic-heavy** → `/tdd`
- **Integration/API** → `/tdd` with contract tests
- **UI/Output formatting** → `/approval-tests`
- **Preparatory** (refactoring, test gaps) → `/tdd` for characterization, then `/refactoring`

Invoke the selected skill. It handles red-green-refactor.

## Outside-In Build Loop

### For each scenario:

1. **Describe the scenario in plain language first** — before any code. Write it as a Given/When/Then:

   ```
   Given [setup]
   When [persona does action]
   Then [what they see or what changes]
   ```

   Present this to the user and get confirmation before proceeding. The acceptance test is a contract — don't decide what the user should see on their behalf.

2. **Turn the confirmed scenario into an acceptance test.** This test will fail. That's the point.

3. **Find the entry point** — where in the system does this scenario begin?
   - Web feature → the UI component where the new affordance appears
   - API feature → the controller/endpoint
   - CLI feature → the command handler

   The acceptance test tells you where to start. The entry point defines what inner layers must provide.

4. **Make the acceptance test pass, outside-in.** Stub everything below — hardcoded data, fake responses, in-memory storage. The entry point should work with stubs before any inner layer exists.

5. **Work inward one layer at a time.** Each layer gets its own tests. Replace the stub from the layer above with real implementation. Then stub the next layer down. Repeat until you reach storage.

6. **Read back code** — is it clear? If not, refactor before moving on.

### Preparatory scenarios (refactoring, test coverage)

1. **Characterization tests first** — if modifying untested code, add tests that capture current behavior before changing anything. These are the safety net for the refactoring that follows.
2. **Refactor** — invoke `/refactoring`. Small steps, tests green after each change.
3. **Verify no regressions** — read back with fresh eyes:
   - Error handling still intact? No swallowed exceptions?
   - Logging still captures key events?
   - No sensitive data exposed through new code paths?
   - Performance characteristics preserved?

## Rationalizations to Watch For

If you find yourself reasoning about why outside-in doesn't apply here, you are rationalizing:

- "The outermost layer has no tests yet" → That's exactly why you start there.
- "The real logic/work is in [deeper layer]" → Outside-in discovers what inner layers need. Inside-out guesses. You're drawn to where business rules live because it feels like "real work." This is backwards.
- "I'll be pragmatic" → Outside-in IS pragmatic. It catches integration issues early and prevents building the wrong API.
- "The backend needs to exist first" → No. That's what stubs are for.
- "This component already exists, so I can skip this layer" → The outermost layer is the component that USES the shared one. Start there.

## Stop If You Catch Yourself

These are not suggestions. Stop immediately and correct course:

- **Adding to long functions** — extract first, then add
- **Copy-pasting with modifications** — extract shared logic
- **Skipping tests "temporarily"** — write the test first
- **Building bottom-up** — start from user interaction, work inward
- **Ordering work inside-out** — if your plan reads "infrastructure → service → controller → UI," reverse it
- **"I'll refactor later"** — refactor now while context is fresh

## Scenario Complete Gate

**All four** checks must pass before moving to next scenario:

1. All tests pass
2. Invoke `/code-review` on the scenario's changes. All violations must be resolved.
3. Update tracking:
   - **Preparatory**: Mark item complete in scout report
   - **Feature**: Mark satisfied criteria in criteria file
4. Commit with message describing the change

## Course Correction

After each scenario, before starting the next:

- Does what you learned invalidate remaining scenarios? → re-sketch remaining work only
- Is an acceptance criterion wrong or ambiguous? → clarify with the user

Completed work stays — only adjust what's ahead.
