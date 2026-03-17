# Scenario 5: Two trait impls, same contract, different files

The #77 case. Two types implement the same trait. Their structural similarity
comes from the contract, not copy-paste. Should be excluded.

```rust
// a.rs
impl Render for Html {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str("<div>");
        buf
    }
}

// b.rs
impl Render for Xml {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str("<root>");
        buf
    }
}
```

The impl block is a language construct that groups functions under a contract.
Our model only cares about the relationship: this code implements this
contract. Like Class in scenario 4, the impl block is transparent. The
Contract node directly contains the tokens.

## Parse

```
Source("a.rs")
  └── Contract("Render")
        ├── Token("fn")
        ├── Token("$ID")
        ├── Token("(")
        ├── Token("&")
        ├── Token("$ID")
        ├── ...

Source("b.rs")
  └── Contract("Render")
        ├── Token("fn")
        ├── Token("$ID")
        ├── ...
```

## Detect

Flatten, suffix array, match.

## Evaluate

Walk up from all occurrences. Both hit Contract("Render"). Same contract
name → **exclude**.

"Same contract" is string equality: `"Render" == "Render"`.

## Note: contract name resolution (parsing concern)

**Qualification:** `impl Render` (imported) vs `impl crate::render::Render`
(fully qualified) would produce different contract names. Tree-sitter gives
exactly what's written. Canonicalizing requires tracing `use` statements.

**Collision:** `renderer::Render` vs `serializer::Render` both imported as
`Render` would incorrectly match. False negative.

Pragmatic start: use the name as written. Low collision risk, and the failure
mode (false negative) is less harmful than false positive.
