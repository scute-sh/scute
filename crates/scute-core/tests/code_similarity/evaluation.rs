use scute_core::Evaluation;
use scute_test_utils::TestDir;

use super::helpers::check_with_low_thresholds;

const RUST_TRAIT_RENDER_HTML: &str = "\
impl Render for Html {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str(\"<div>\");
        buf.push_str(\"</div>\");
        buf
    }
}";

const RUST_TRAIT_RENDER_XML: &str = "\
impl Render for Xml {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str(\"<root>\");
        buf.push_str(\"</root>\");
        buf
    }
}";

const TS_IMPLEMENTS_RENDERER_HTML: &str = "\
class HtmlRenderer implements Renderer {
    render(): string {
        let buf = '';
        buf += '<div>';
        buf += '</div>';
        return buf;
    }
}";

const TS_IMPLEMENTS_RENDERER_XML: &str = "\
class XmlRenderer implements Renderer {
    render(): string {
        let buf = '';
        buf += '<root>';
        buf += '</root>';
        return buf;
    }
}";

const JS_EXTENDS_ABSTRACT_HTML: &str = "\
class HtmlRenderer extends AbstractRenderer {
    render() {
        let buf = '';
        buf += '<div>';
        buf += '</div>';
        return buf;
    }
}";

const JS_EXTENDS_ABSTRACT_XML: &str = "\
class XmlRenderer extends AbstractRenderer {
    render() {
        let buf = '';
        buf += '<root>';
        buf += '</root>';
        return buf;
    }
}";

#[test_case::test_case("a.rs", RUST_TRAIT_RENDER_HTML, "b.rs", RUST_TRAIT_RENDER_XML
    ; "same rust traits")]
#[test_case::test_case("a.ts", TS_IMPLEMENTS_RENDERER_HTML, "b.ts", TS_IMPLEMENTS_RENDERER_XML
    ; "same ts interfaces")]
#[test_case::test_case("a.js", JS_EXTENDS_ABSTRACT_HTML, "b.js", JS_EXTENDS_ABSTRACT_XML
    ; "same js extends")]
#[test_case::test_case(
    "a.ts", TS_IMPLEMENTS_RENDERER_HTML,
    "b.js", "\
class XmlRenderer extends Renderer {
    render() {
        let buf = '';
        buf += '<root>';
        buf += '</root>';
        return buf;
    }
}"
    ; "cross language ts implements and js extends same contract")]
fn excludes_same_contract_clone_groups(
    file_a: &str,
    content_a: &str,
    file_b: &str,
    content_b: &str,
) {
    let dir = TestDir::new()
        .source_file(file_a, content_a)
        .source_file(file_b, content_b);

    let evals = check_with_low_thresholds(&dir.root());

    assert!(
        evals.iter().all(Evaluation::is_pass),
        "same-contract impls should be excluded, got: {evals:?}"
    );
}

#[test]
fn reports_duplication_across_different_contracts() {
    let dir = TestDir::new()
        .source_file("a.rs", RUST_TRAIT_RENDER_HTML)
        .source_file(
            "b.rs",
            "\
impl Format for Xml {
    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str(\"<root>\");
        buf.push_str(\"</root>\");
        buf
    }
}",
        );

    let evals = check_with_low_thresholds(&dir.root());

    assert!(
        evals.iter().any(|e| !e.is_pass()),
        "expected duplication reported, got: {evals:?}"
    );
}

#[test_case::test_case("tests/a.rs", "tests/b.rs",
    "fn foo(x: i32) -> i32 { x + 1 }",
    "fn bar(y: i32) -> i32 { y + 1 }"
    ; "rust test directory"
)]
#[test_case::test_case("a.test.ts", "b.test.ts",
    "function foo(x: number): number { return x + 1; }",
    "function bar(y: number): number { return y + 1; }"
    ; "typescript test files"
)]
#[test_case::test_case("src/a.rs", "src/b.rs",
    "fn serve() -> String { String::from(\"hello\") }\n\
     #[cfg(test)]\nmod tests {\n    fn helper_a(x: i32) -> i32 { x + 1 }\n}",
    "use std::collections::HashMap;\n\
     #[cfg(test)]\nmod tests {\n    fn helper_b(y: i32) -> i32 { y + 1 }\n}"
    ; "inline rust test modules"
)]
fn applies_test_thresholds(file_a: &str, file_b: &str, content_a: &str, content_b: &str) {
    let dir = TestDir::new()
        .source_file(file_a, content_a)
        .source_file(file_b, content_b);

    let evals = check_with_low_thresholds(&dir.root());

    // Clone pair produces 14 tokens. Test thresholds: warn=10, fail=30.
    // 14 > 10 (warn) but 14 < 30 (fail), so the result should be warn.
    assert!(
        evals[0].is_warn(),
        "expected warn (test thresholds), got: {evals:?}"
    );
}

#[test]
fn excludes_flat_string_literal_list() {
    let dir = TestDir::new().source_file(
        "slugs.ts",
        "\
const RESERVED = new Set([
    'api', 'app', 'maps', 'embed', 'share', 'edit', 'create',
    'dashboard', 'home', 'explore', 'search', 'auth', 'login',
    'logout', 'signup', 'register', 'verify', 'reset', 'oauth',
    'profile', 'account', 'settings', 'admin', 'billing', 'plan',
    'about', 'contact', 'help', 'support', 'docs', 'blog', 'news',
    'terms', 'privacy', 'legal', 'status', 'health', 'metrics',
    'developer', 'sdk', 'cli', 'tools', 'webhooks', 'graphql',
    'www', 'mail', 'cdn', 'assets', 'static', 'files', 'uploads',
    'test', 'staging', 'prod', 'sandbox', 'beta', 'preview',
]);",
    );

    let evals = check_with_low_thresholds(&dir.root());

    assert!(
        evals.iter().all(Evaluation::is_pass),
        "flat literal list should not be flagged, got: {evals:?}"
    );
}

#[test]
fn excludes_flat_number_literal_list() {
    let dir = TestDir::new().source_file(
        "ports.ts",
        "\
const RESERVED_PORTS = [
    80, 443, 8080, 8443, 3000, 3001, 5000, 5001,
    9090, 9091, 4000, 4001, 6000, 6001, 7000, 7001,
    2000, 2001, 1234, 5678, 9999, 1111, 2222, 3333,
];",
    );

    let evals = check_with_low_thresholds(&dir.root());

    assert!(
        evals.iter().all(Evaluation::is_pass),
        "flat number literal list should not be flagged, got: {evals:?}"
    );
}

#[test]
fn detects_identical_literal_lists_across_files() {
    let list = "\
const RESERVED = new Set([
    'api', 'app', 'maps', 'embed', 'share', 'edit', 'create',
    'dashboard', 'home', 'explore', 'search', 'auth', 'login',
    'logout', 'signup', 'register', 'verify', 'reset', 'oauth',
    'profile', 'account', 'settings', 'admin', 'billing', 'plan',
]);";
    let dir = TestDir::new()
        .source_file("a.ts", list)
        .source_file("b.ts", list);

    let evals = check_with_low_thresholds(&dir.root());

    assert!(
        evals.iter().any(|e| !e.is_pass()),
        "identical lists in separate files should be detected, got: {evals:?}"
    );
}
