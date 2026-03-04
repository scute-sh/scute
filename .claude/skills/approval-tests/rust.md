# Rust Approval Tests (insta)

## Installation

```toml
# Cargo.toml
[dev-dependencies]
insta = "1"
```

For JSON snapshots or redactions:
```toml
insta = { version = "1", features = ["json", "redactions"] }
```

Optional CLI for reviewing snapshots:
```bash
cargo install cargo-insta
```

## Quick Start

```rust
use insta::assert_snapshot;

#[test]
fn generates_report() {
    let result = generate_report();
    assert_snapshot!(result);
}
```

**First run:** Test fails, `.snap.new` file created. Review it with
`cargo insta review`, accept it (becomes `.snap`), rerun.

## Core Patterns

### assert_snapshot!() - String verification
```rust
// Named snapshot (recommended for clarity)
assert_snapshot!("report_output", result);

// Auto-named from test function name
assert_snapshot!(result);
```

### assert_debug_snapshot!() - Debug output of any type
```rust
assert_debug_snapshot!(users);
```

### assert_json_snapshot!() - Objects as formatted JSON
```rust
// Requires `json` feature
use insta::assert_json_snapshot;

assert_json_snapshot!(user);
```

### assert_yaml_snapshot!() - Objects as YAML
```rust
assert_yaml_snapshot!(config);
```

### Inline snapshots
Expectations live in the source code instead of separate files:
```rust
assert_snapshot!(result, @"expected output here");
// insta fills in the @"..." on first run
```

### Multiple snapshots in one test
Use distinct names to avoid overwriting:
```rust
assert_snapshot!("before", state.to_string());
state.advance();
assert_snapshot!("after", state.to_string());
```

### Redactions (scrubbing)
Replace non-deterministic values:
```rust
// Requires `redactions` feature
use insta::{assert_json_snapshot, with_settings};

with_settings!({
    filters => vec![
        (r"\d{4}-\d{2}-\d{2}", "[DATE]"),
        (r"[0-9a-f]{8}-[0-9a-f]{4}", "[UUID]"),
    ]
}, {
    assert_snapshot!(result);
});

// Or with JSON path selectors
assert_json_snapshot!(value, {
    ".timestamp" => "[timestamp]",
    ".id" => "[id]",
});
```

### Settings for test modules
```rust
use insta::Settings;

fn settings() -> Settings {
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("testdata/snapshots");
    settings
}

#[test]
fn my_test() {
    let _guard = settings().bind_to_scope();
    assert_snapshot!(result);
}
```

## Workflow

```
cargo test                    # run tests, .snap.new created for new/changed
cargo insta review            # interactive review of pending snapshots
cargo insta accept --all      # accept all pending (when confident)
cargo insta reject --all      # reject all pending
```

Without `cargo-insta`, manually rename `.snap.new` to `.snap`.

## Git Setup

```gitignore
*.snap.new
```

Commit all `.snap` files — they are your approved expectations.

## Rust-Specific Notes

**Display vs Debug** — `assert_snapshot!` uses `Display` (or raw string).
`assert_debug_snapshot!` uses `Debug`. Pick the one that produces the most
readable output for review.

**Serialization** — JSON/YAML snapshots require `Serialize` on your types.
For types you don't own, use `assert_debug_snapshot!` or format manually.

**Inline snapshots** — Great for short output. `cargo insta review` or
`cargo fmt` auto-fills the expected value. Avoids file proliferation.

**Snapshot location** — By default, snapshots live in a `snapshots/`
directory next to the test file. Configurable via `Settings`.
