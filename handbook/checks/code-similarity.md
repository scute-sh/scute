# Code Similarity — Design Notes

## The problem

Find structurally duplicated code to flag maintenance risks. Duplication
means "if you change this, you probably need to change that too." That's
the signal we're after.

But not all similarity is a maintenance risk. Some code is similar
because it's _supposed_ to be:

- **Test code** follows Arrange-Act-Assert. Similar setup, similar
  assertions. That's the nature of tests, not a sign of copy-paste.
- **Contract implementations** (traits, interfaces, abstract classes)
  share the same shape because the contract demands it. Two types
  implementing the same interface will naturally have similar method
  signatures and structural patterns.

Flagging expected similarity produces noise that blocks legitimate code
and erodes trust in the check.

## The model: similarity in context

The core insight: **the same degree of similarity means different things
depending on where the code lives.** Two free functions with identical
structure are probably copy-paste. Two implementations of the same trait
with identical structure is the contract doing its job.

This makes "structural context" a first-class concept. A context is a
code region where similarity is expected by design. The check should
understand what context code lives in and evaluate accordingly.

### Known contexts

| Context                 | Why similarity is expected                             |
| ----------------------- | ------------------------------------------------------ |
| Production code         | No inherent expectation, similarity = maintenance risk |
| Test code               | AAA pattern, fixture repetition, assertion boilerplate |
| Contract implementation | The interface demands the same shape                   |

This list will grow. Generated code, macro expansions, and protocol
implementations are all candidates. The design should accommodate new
contexts without ad-hoc mechanisms per type.

### Principles

1. **Context flows forward.** Structural information about code regions
   should be captured when it's naturally available, not recovered later
   by re-analyzing the same source. Parse once, carry context through.

2. **Detection finds, evaluation judges.** Detection is context-blind:
   it finds structurally similar code. Evaluation is context-aware: it
   decides whether that similarity is a problem. These are separate
   concerns.
