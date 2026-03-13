---
name: harden
description: "Hardens code for production: edge cases, resilience, security, and polish. Use when preparing code for release, checking production-readiness, or hardening a feature."
---

# Harden

STARTER_CHARACTER = 🛡️

Production-readiness pass. Complete **all four** steps.

## 1. Edge Cases (ZOMBIES)

Walk through **every** category, add tests for gaps found:

- **Z**ero/empty — null, empty string, zero, empty collection
- **O**ne — single item
- **M**any — multiple items, ordering
- **B**oundaries — min/max, off-by-one
- **I**nterfaces — API contracts, type precision
- **E**xceptions — error paths, failure modes
- **S**imple — obvious cases easily overlooked

## 2. Production Checklist

See [references/production-checklist.md](references/production-checklist.md) for full checklist.

Address **every** item (mark N/A with reason if not applicable):

- [ ] Error handling: graceful degradation, meaningful messages
- [ ] Logging: key events, debug info, no sensitive data
- [ ] Observability and analytics: can you tell if it's working and valuable?
- [ ] Resilience: timeouts, retries where appropriate
- [ ] Performance: latency acceptable, no obvious bottlenecks
- [ ] Security: OWASP Top 10 walkthrough
- [ ] Accessibility (if user-facing)

## 3. Refactoring Pass

Invoke `/refactoring` on all files touched. This is a required step, not optional.

## 4. Writing Style Review

Two parts: coverage check, then style review.

**Coverage** — check that these exist where they should. Missing text is worse than imperfect text:
- Public API docs for every exported function/type
- UI text: labels, tooltips, empty states, confirmation dialogs, onboarding hints
- Error messages for every new failure path — user-facing and developer-facing
- README or handbook updates if the feature changes user-visible behavior
- Doc examples that compile/run
- Changelog entry if the project keeps one

**Style** — invoke `/writing-style` on all text added or modified. This includes error messages, log messages, API docs, CLI output — not just UI copy.

## Gate

**All five** checks must pass:

- [ ] ZOMBIES walked through every category, tests added for gaps
- [ ] Production checklist: every item addressed (or N/A with reason)
- [ ] Refactoring pass complete
- [ ] Writing style reviewed
- [ ] All tests pass
