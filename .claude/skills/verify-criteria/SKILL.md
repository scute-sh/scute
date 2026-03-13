---
name: verify-criteria
description: Verifies every acceptance criterion is met with test evidence and runs final checks. Use when verifying a feature is complete, demonstrating criteria are satisfied, or doing a final review before shipping.
---

# Verify Criteria

STARTER_CHARACTER = ✅

Confirm the work is complete and correct.

## Criteria Demonstration

For **every** acceptance criterion:

1. State the criterion
2. Cite the test(s) that prove it. If the criterion can't be verified by a test, explain why and provide alternative evidence.
3. Mark VERIFIED or FAILED

If any criterion is FAILED, go back and fix it.

## Final Checks

Complete **all five**:

1. Run full test suite — all must pass
2. Review all commits — do they tell a coherent story?
3. Self-review: read the diff as someone else's code. Address any comments you'd leave.
4. What could break in production that tests don't cover?
5. Update docs if the feature affects public APIs, user-facing behavior, or onboarding

## Gate

**All five** checks must pass:

- [ ] Every acceptance criterion VERIFIED with evidence
- [ ] All tests pass
- [ ] Self-review complete, no outstanding concerns
- [ ] Documentation updated (or N/A)
- [ ] Summary written: what was built, key decisions, trade-offs
