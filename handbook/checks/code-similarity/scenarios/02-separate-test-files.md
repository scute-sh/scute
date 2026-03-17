# Scenario 2: Similar functions in separate test files

First context scenario. Both files are test code. Should still be flagged,
but with higher thresholds.

```rust
// tests/a.rs
fn test_html() {
    let result = render_html();
    assert_eq!(result, "<div>");
}

// tests/b.rs
fn test_xml() {
    let result = render_xml();
    assert_eq!(result, "<root>");
}
```

## Parse

```
Source("tests/a.rs")
  └── TestRegion
        ├── Token("fn")
        ├── Token("$ID")
        ...

Source("tests/b.rs")
  └── TestRegion
        ├── Token("fn")
        ├── Token("$ID")
        ...
```

A source file is neutral. It doesn't carry context. The test context is a
structural region inside the file. Whether we determine "this is test code"
from the file path (`tests/`) or from source attributes (`#[test]`), the
representation is the same: a TestRegion that wraps the tokens.

## Detect

Flatten, suffix array, match.

## Evaluate

Walk up from all occurrences. The test rule only applies when every occurrence
is inside a TestRegion. Both hit TestRegion → test thresholds → **warn**.
