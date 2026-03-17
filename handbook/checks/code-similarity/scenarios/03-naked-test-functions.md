# Scenario 3: Naked test functions alongside production code

Test and production code coexist in the same file with no module boundary.
The `#[test]` attribute is the only marker.

```rust
// src/render.rs
fn render_html() -> String {
    let mut buf = String::new();
    buf.push_str("<div>");
    buf
}

#[test]
fn test_html() {
    let result = render_html();
    assert_eq!(result, "<div>");
}

// src/format.rs
fn render_xml() -> String {
    let mut buf = String::new();
    buf.push_str("<root>");
    buf
}

#[test]
fn test_xml() {
    let result = render_xml();
    assert_eq!(result, "<root>");
}
```

Two clone pairs: `render_*` (production) and `test_*` (test).

## Parse

```
Source("src/render.rs")
  ├── Token("fn")          ← render_html tokens
  ├── Token("$ID")
  ├── ...
  └── TestRegion
        ├── Token("fn")    ← test_html tokens
        ├── Token("$ID")
        ├── ...

Source("src/format.rs")
  ├── Token("fn")          ← render_xml tokens
  ├── Token("$ID")
  ├── ...
  └── TestRegion
        ├── Token("fn")    ← test_xml tokens
        ├── Token("$ID")
        ├── ...
```

A `#[test]` function is contained in a test region. Whether the source
expresses that via a module wrapper, a file path, or an attribute is a
syntactic detail. Our model normalizes all of them: the structural reality
is that this code lives in test context.

## Detect

Flatten, suffix array. Finds both clone pairs.

## Evaluate

For each clone group, walk up from ALL occurrences. The test rule only applies
when every occurrence is inside a TestRegion. If any one isn't, the rule
doesn't match and we fall through to standard thresholds.

- render_html ↔ render_xml: walk up from both → Token → Source. No
  TestRegion → standard thresholds → **fail**.
- test_html ↔ test_xml: walk up from both → Token → TestRegion → Source.
  Both inside TestRegion → test thresholds → **warn**.
