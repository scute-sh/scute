# Scenario 1: Two similar free functions

The baseline. No special context. Should be flagged as duplication.

```rust
// a.rs
fn foo(x: i32) -> i32 { x + 1 }

// b.rs
fn bar(y: i32) -> i32 { y + 1 }
```

## Parse

```
Source("a.rs")
  ├── Token("fn")
  ├── Token("$ID")
  ├── Token("(")
  ├── Token("$ID")
  ├── Token(":")
  ├── Token("$ID")
  ├── Token(")")
  ├── Token("->")
  ├── Token("$ID")
  ├── Token("{")
  ├── Token("$ID")
  ├── Token("+")
  ├── Token("$LIT")
  └── Token("}")

Source("b.rs")
  ├── Token("fn")
  ...same structure...
  └── Token("}")
```

No context containers. Tokens are directly under Source.

## Detect

Flatten leaves from both trees into one token sequence with sentinels. Suffix
array finds 14-token match.

## Evaluate

For each token in the match, walk up: Token → Source. No TestRegion, no
Contract. Regular code → standard thresholds → 14 tokens > fail → **fail**.
