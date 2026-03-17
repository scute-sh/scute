# Scenario 6: Contract implementations in test code

Test doubles (fakes, stubs, mocks) that implement the same contract as
production code. Two contexts overlap: test and contract.

## Case A: production impl + test fake, same contract

```rust
// src/html.rs
impl Render for HtmlRenderer {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str("<div>");
        buf
    }
}

// tests/fakes.rs
impl Render for FakeRenderer {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str("<fake>");
        buf
    }
}
```

### Parse

```
Source("src/html.rs")
  └── Contract("Render")
        ├── Token("fn")
        ├── Token("$ID")
        ├── ...

Source("tests/fakes.rs")
  └── TestRegion
        └── Contract("Render")
              ├── Token("fn")
              ├── Token("$ID")
              ├── ...
```

Nested context: Contract inside TestRegion. Tokens in tests/fakes.rs have
two ancestors that carry context.

### Evaluate

Walk up from both occurrences. Both hit Contract("Render"). Same contract →
**exclude**.

The TestRegion above the Contract in tests/fakes.rs is irrelevant because the
contract rule takes priority. Rule ordering matters: contract rule runs first,
and if it matches, the test rule never applies.

## Case B: two test fakes, same contract

```rust
// tests/fakes.rs
impl Render for FakeHtml {
    fn render(&self) -> String { ... }
}

// tests/stubs.rs
impl Render for StubRenderer {
    fn render(&self) -> String { ... }
}
```

### Parse

```
Source("tests/fakes.rs")
  └── TestRegion
        └── Contract("Render")
              ├── Token(...)

Source("tests/stubs.rs")
  └── TestRegion
        └── Contract("Render")
              ├── Token(...)
```

### Evaluate

Both hit Contract("Render"). Same contract → **exclude**. Both also inside
TestRegion, but contract rule already resolved it.

## Takeaway

Overlapping contexts work naturally. The tree nesting reflects the source
structure (a contract impl that lives in test code), and rule priority
determines which context wins. The data structure doesn't impose priority;
the rule engine does.
