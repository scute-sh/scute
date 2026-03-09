use scute_core::code_similarity::{CloneGroup, LanguageConfig, SourceEntry, find_clones, language};

use super::helpers::snapshot;

const LOW_TOKEN_THRESHOLD: usize = 10;

fn find_clone_groups(files: &[(&str, &str, &LanguageConfig)]) -> Vec<CloneGroup> {
    let entries: Vec<_> = files
        .iter()
        .map(|(content, path, lang)| SourceEntry::new(content, path, lang))
        .collect();
    find_clones(&entries, LOW_TOKEN_THRESHOLD).unwrap()
}

#[test]
fn detects_duplication_across_rust_files() {
    let file_a = "\
fn validate_email(input: &str) -> Result<String, Error> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::Empty);
    }
    if !trimmed.contains('@') {
        return Err(Error::Invalid);
    }
    Ok(trimmed.to_string())
}";

    let file_b = "\
fn validate_username(name: &str) -> Result<String, Error> {
    let cleaned = name.trim();
    if cleaned.is_empty() {
        return Err(Error::Empty);
    }
    if !cleaned.contains(' ') {
        return Err(Error::Invalid);
    }
    Ok(cleaned.to_string())
}";

    let rust = language::rust();
    let groups = find_clone_groups(&[
        (file_a, "validators/email.rs", &rust),
        (file_b, "validators/username.rs", &rust),
    ]);

    insta::assert_snapshot!(snapshot(&groups));
}

#[test]
fn detects_duplication_across_typescript_files() {
    let file_a = "\
export async function fetchUser(id: string): Promise<User> {
  const response = await fetch(`/api/users/${id}`);
  if (!response.ok) {
    throw new Error('Request failed');
  }
  return response.json();
}";

    let file_b = "\
export async function fetchOrder(orderId: number): Promise<Order> {
  const res = await fetch(`/api/orders/${orderId}`);
  if (!res.ok) {
    throw new Error('Network error');
  }
  return res.json();
}";

    let ts = language::typescript();
    let groups = find_clone_groups(&[
        (file_a, "api/users.ts", &ts),
        (file_b, "api/orders.ts", &ts),
    ]);

    insta::assert_snapshot!(snapshot(&groups));
}

#[test]
fn mixed_languages_detect_within_same_language() {
    let rust_a = "fn process(x: i32) -> i32 { x * 2 + 1 }";
    let rust_b = "fn transform(y: u64) -> u64 { y * 2 + 1 }";
    let ts_code = "function compute(n: number): number { return n * 2 + 1; }";

    let rust = language::rust();
    let ts = language::typescript();
    // Rust files should match each other; TS has different token structure
    // so it may or may not match (cross-language is out of scope, but tokens
    // might coincidentally align). Snapshot captures the actual behavior.
    let groups = find_clone_groups(&[
        (rust_a, "a.rs", &rust),
        (rust_b, "b.rs", &rust),
        (ts_code, "c.ts", &ts),
    ]);

    insta::assert_snapshot!(snapshot(&groups));
}

#[test]
fn multi_file_project_with_mixed_duplication() {
    // Two validators share full structure (cross-file clone)
    let validate_email = "\
fn validate_email(input: &str) -> Result<String, Error> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::Empty);
    }
    if !trimmed.contains('@') {
        return Err(Error::Invalid);
    }
    Ok(trimmed.to_string())
}";

    let validate_phone = "\
fn validate_phone(raw: &str) -> Result<String, Error> {
    let cleaned = raw.trim();
    if cleaned.is_empty() {
        return Err(Error::Empty);
    }
    if !cleaned.contains('+') {
        return Err(Error::Invalid);
    }
    Ok(cleaned.to_string())
}";

    // One file has within-file duplication (two similar handlers)
    let handlers = "\
fn handle_create(input: &str) -> Result<String, Error> {
    let parsed = input.trim();
    if parsed.is_empty() {
        return Err(Error::Empty);
    }
    Ok(parsed.to_string())
}

fn handle_update(data: &str) -> Result<String, Error> {
    let cleaned = data.trim();
    if cleaned.is_empty() {
        return Err(Error::Empty);
    }
    Ok(cleaned.to_string())
}";

    // Unique file: completely different structure, should NOT appear
    let config = "\
struct Config {
    host: String,
    port: u16,
    max_retries: u32,
}

impl Config {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 8080,
            max_retries: 3,
        }
    }
}";

    let rust = language::rust();
    let groups = find_clone_groups(&[
        (validate_email, "validators/email.rs", &rust),
        (validate_phone, "validators/phone.rs", &rust),
        (handlers, "handlers.rs", &rust),
        (config, "config.rs", &rust),
    ]);

    insta::assert_snapshot!(snapshot(&groups));
}

#[test]
fn empty_entries_returns_no_clones() {
    let groups = find_clones(&[], LOW_TOKEN_THRESHOLD).unwrap();
    assert!(groups.is_empty());
}
