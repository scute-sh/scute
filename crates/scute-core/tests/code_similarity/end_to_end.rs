use scute_core::code_similarity::{
    detect_clones, javascript::JsFamily, rules::SimilarityRules, rust::Rust,
};

use super::helpers::{parse_and_detect, snapshot};

const LOW_TOKEN_THRESHOLD: usize = 10;

fn find_clone_groups(
    files: &[(&str, &str, &dyn SimilarityRules)],
) -> super::helpers::DetectionResult {
    parse_and_detect(files, LOW_TOKEN_THRESHOLD)
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

    let result = find_clone_groups(&[
        (file_a, "validators/email.rs", &Rust),
        (file_b, "validators/username.rs", &Rust),
    ]);

    insta::assert_snapshot!(snapshot(&result));
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

    let ts = JsFamily::typescript();
    let result = find_clone_groups(&[
        (file_a, "api/users.ts", &ts),
        (file_b, "api/orders.ts", &ts),
    ]);

    insta::assert_snapshot!(snapshot(&result));
}

#[test]
fn ignores_cross_language_matches() {
    let rust_a = "fn process(x: i32) -> i32 { x * 2 + 1 }";
    let rust_b = "fn transform(y: u64) -> u64 { y * 2 + 1 }";
    let ts_code = "function compute(n: number): number { return n * 2 + 1; }";

    let ts = JsFamily::typescript();
    let result = find_clone_groups(&[
        (rust_a, "a.rs", &Rust),
        (rust_b, "b.rs", &Rust),
        (ts_code, "c.ts", &ts),
    ]);

    insta::assert_snapshot!(snapshot(&result));
}

#[test]
fn multi_file_project_with_mixed_duplication() {
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

    let result = find_clone_groups(&[
        (validate_email, "validators/email.rs", &Rust),
        (validate_phone, "validators/phone.rs", &Rust),
        (handlers, "handlers.rs", &Rust),
        (config, "config.rs", &Rust),
    ]);

    insta::assert_snapshot!(snapshot(&result));
}

#[test]
fn empty_entries_returns_no_clones() {
    let groups = detect_clones(&[], LOW_TOKEN_THRESHOLD);
    assert!(groups.is_empty());
}
