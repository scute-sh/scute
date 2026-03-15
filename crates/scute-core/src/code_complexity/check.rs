use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::rules::LanguageRules;
use super::{rust, score, typescript};
use crate::files;
use crate::{Evaluation, Evidence, ExecutionError, Expected, Thresholds};

pub const CHECK_NAME: &str = "code-complexity";

/// Configuration for the code complexity check.
///
/// All fields are optional and fall back to sensible defaults when absent.
///
/// ```
/// use scute_core::code_complexity::Definition;
///
/// // Zero-config: warn at 5, fail at 10
/// let default = Definition::default();
/// ```
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Definition {
    /// Warn/fail boundaries for per-function complexity scores.
    /// Defaults to warn: 5, fail: 10.
    pub thresholds: Option<Thresholds>,
    /// Glob patterns for files to exclude from scanning.
    pub exclude: Option<Vec<String>>,
}

impl Definition {
    fn thresholds(&self) -> Thresholds {
        self.thresholds.clone().unwrap_or(Thresholds {
            warn: Some(5),
            fail: Some(10),
        })
    }
}

/// Score cognitive complexity for every function in the given paths.
///
/// Accepts a mix of files and directories. Directories are walked to
/// discover supported files (respecting `exclude` patterns).
///
/// Returns one [`Evaluation`] per function found. When no supported files
/// exist, returns a single passing evaluation.
///
/// ```no_run
/// use std::path::PathBuf;
/// use scute_core::code_complexity::{self, Definition};
///
/// let evals = code_complexity::check(
///     &[PathBuf::from("src/")],
///     &Definition::default(),
/// ).unwrap();
/// for eval in &evals {
///     if eval.is_fail() {
///         eprintln!("complex function: {}", eval.target);
///     }
/// }
/// ```
///
/// # Errors
///
/// Returns `ExecutionError` if any path is invalid.
pub fn check(
    paths: &[PathBuf],
    definition: &Definition,
) -> Result<Vec<Evaluation>, ExecutionError> {
    let thresholds = definition.thresholds();
    let exclude = definition.exclude.as_deref().unwrap_or_default();

    let extensions = &["rs", "ts", "tsx"];
    let files = files::resolve_paths(paths, extensions, exclude).map_err(|e| ExecutionError {
        code: "invalid_target".into(),
        message: e.to_string(),
        recovery: "check that the path exists and is readable".into(),
    })?;

    let rust = rust::Rust;
    let typescript =
        typescript::TypeScript::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
    let tsx = typescript::TypeScript::new(tree_sitter_typescript::LANGUAGE_TSX.into());

    let mut evaluations = Vec::new();

    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let rules: &dyn LanguageRules = match path.extension().and_then(|e| e.to_str()) {
            Some("ts") => &typescript,
            Some("tsx") => &tsx,
            _ => &rust,
        };
        evaluations.extend(score_file(path, &source, rules, &thresholds));
    }

    if evaluations.is_empty() {
        let label = paths
            .first()
            .map_or_else(|| ".".into(), |p| p.display().to_string());
        evaluations.push(Evaluation::completed(label, 0, thresholds, vec![]));
    }

    Ok(evaluations)
}

fn score_file(
    path: &Path,
    source: &str,
    rules: &dyn LanguageRules,
    thresholds: &Thresholds,
) -> Vec<Evaluation> {
    score::score_functions(source, rules)
        .into_iter()
        .map(|func| {
            let target = format!("{}:{}:{}", path.display(), func.line, func.name);
            let evidence = func
                .contributors
                .iter()
                .map(|c| format_evidence(c, path))
                .collect();
            Evaluation::completed(target, func.score, thresholds.clone(), evidence)
        })
        .collect()
}

fn format_nesting_chain(chain: &[score::FlowConstruct]) -> String {
    chain
        .iter()
        .map(|c| c.label)
        .collect::<Vec<_>>()
        .join(" > ")
}

fn pluralize_levels(n: u64) -> &'static str {
    if n == 1 { "level" } else { "levels" }
}

fn format_ops(operators: &[score::LogicalOp]) -> String {
    let mut unique: Vec<&str> = operators.iter().map(|o| o.label()).collect();
    unique.dedup();
    let quoted: Vec<String> = unique.iter().map(|o| format!("'{o}'")).collect();
    let prefix = if unique.len() > 1 { "mixed " } else { "" };
    format!("{prefix}{}", quoted.join(" and "))
}

fn format_evidence(c: &score::Contributor, path: &Path) -> Evidence {
    let location = Some(format!("{}:{}", path.display(), c.line));
    let text = |s: &str| Some(Expected::Text(s.into()));

    let (rule, found, expected) = match &c.kind {
        score::ContributorKind::FlowBreak { construct } => (
            "flow break",
            format!(
                "'{}' {} (+{})",
                construct.label,
                construct.role.flow_break_category(),
                c.increment
            ),
            None,
        ),
        score::ContributorKind::Nesting {
            construct,
            depth,
            chain,
        } => {
            let name = construct.label;
            let chain = format_nesting_chain(chain);
            let levels = pluralize_levels(*depth);
            (
                "nesting",
                format!(
                    "'{name}' nested {depth} {levels}: '{chain}' (+{})",
                    c.increment
                ),
                text("extract inner block into a function"),
            )
        }
        score::ContributorKind::Else => (
            "else",
            format!("'else' branch (+{})", c.increment),
            text("use a guard clause or early return"),
        ),
        score::ContributorKind::Logical { operators } => (
            "boolean logic",
            format!("{} operators (+{})", format_ops(operators), c.increment),
            text("extract into a named boolean"),
        ),
        score::ContributorKind::Recursion { fn_name } => (
            "recursion",
            format!("recursive call to '{fn_name}' (+{})", c.increment),
            text("consider iterative approach"),
        ),
        score::ContributorKind::Jump { keyword, label } => (
            "jump",
            format!("'{}' to label {label} (+{})", keyword.label(), c.increment),
            text("restructure to avoid labeled jump"),
        ),
    };

    Evidence {
        rule: Some(rule.to_string()),
        location,
        found,
        expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scute_test_utils::TestDir;
    use test_case::test_case;

    fn check_dir(dir: &Path) -> Vec<Evaluation> {
        check(&[dir.to_path_buf()], &Definition::default()).unwrap()
    }

    #[test]
    fn returns_one_evaluation_per_function() {
        let dir = TestDir::new().source_file(
            "two.rs",
            "fn a() {} fn b(x: i32) -> i32 { if x > 0 { 1 } else { -1 } }",
        );

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 2);
        assert!(evals[0].target.contains('a'));
        assert!(evals[1].target.contains('b'));
    }

    #[test]
    fn returns_single_pass_for_empty_directory() {
        let dir = TestDir::new();

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass());
    }

    #[test]
    fn scores_only_functions_in_specified_file() {
        let dir = TestDir::new()
            .source_file("target.rs", "fn focused() { if true {} }")
            .source_file("other.rs", "fn ignored() { if true {} }");

        let evals = check(&[dir.path("target.rs")], &Definition::default()).unwrap();

        assert!(evals.iter().all(|e| e.target.contains("focused")));
    }

    #[test]
    fn applies_default_thresholds() {
        let dir = TestDir::new().source_file("simple.rs", "fn f() { if true {} }");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass()); // score 1, default warn 5
    }

    fn evidence_of(source: &str) -> Vec<Evidence> {
        let dir = TestDir::new().source_file("a.rs", source);

        let mut evals = check_dir(&dir.root());
        let crate::Outcome::Completed { evidence, .. } = evals.remove(0).outcome else {
            panic!("expected completed");
        };
        evidence
    }

    #[test_case(
        "fn f() { if true {} }",
        "flow break", "'if' conditional (+1)", None
        ; "flow_break_has_no_suggestion"
    )]
    #[test_case(
        "fn f() { for x in [1] { if true {} } }",
        "nesting", "'if' nested 1 level: 'for > if' (+2)", Some("extract inner block into a function")
        ; "nesting_shows_chain_and_suggests_extraction"
    )]
    #[test_case(
        "fn f(x: bool) { if x {} else {} }",
        "else", "'else' branch (+1)", Some("use a guard clause or early return")
        ; "else_suggests_guard_clause"
    )]
    #[test_case(
        "fn f(a: bool, b: bool) -> bool { a && b }",
        "boolean logic", "'&&' operators (+1)", Some("extract into a named boolean")
        ; "logical_single_operator"
    )]
    #[test_case(
        "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }",
        "boolean logic", "mixed '&&' and '||' operators (+2)", Some("extract into a named boolean")
        ; "logical_mixed_operators"
    )]
    #[test_case(
        "fn go(n: u64) -> u64 { go(n - 1) }",
        "recursion", "recursive call to 'go' (+1)", Some("consider iterative approach")
        ; "recursion_shows_function_name"
    )]
    #[test_case(
        "fn f() { 'outer: loop { break 'outer; } }",
        "jump", "'break' to label 'outer (+1)", Some("restructure to avoid labeled jump")
        ; "jump_shows_label"
    )]
    fn evidence_formatting(source: &str, rule: &str, expected_found: &str, expected: Option<&str>) {
        let evidence = evidence_of(source);
        let entry = evidence
            .iter()
            .find(|e| e.rule.as_deref() == Some(rule))
            .unwrap_or_else(|| panic!("no evidence with rule '{rule}'"));

        assert_eq!(
            entry.found, expected_found,
            "evidence found mismatch for rule '{rule}'"
        );
        assert_eq!(entry.expected, expected.map(|s| Expected::Text(s.into())));
    }

    #[test]
    fn evidence_includes_file_location() {
        let evidence = evidence_of("fn f() { if true {} }");

        assert!(evidence[0].location.as_ref().unwrap().contains("a.rs:1"));
    }

    #[test]
    fn rejects_nonexistent_path() {
        let result = check(&[PathBuf::from("/does/not/exist")], &Definition::default());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "invalid_target");
    }

    #[test]
    fn skips_non_rust_files() {
        let dir = TestDir::new().source_file("code.py", "def foo(): pass");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass()); // fallback pass, no rust files
    }

    #[test]
    fn rejects_nonexistent_file() {
        let result = check(
            &[PathBuf::from("/nonexistent/file.rs")],
            &Definition::default(),
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "invalid_target");
    }

    #[test]
    fn rejects_unsupported_file_extension() {
        let dir = TestDir::new().source_file("code.py", "def foo(): pass");

        let result = check(&[dir.path("code.py")], &Definition::default());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "invalid_target");
    }

    #[test]
    fn scores_typescript_function() {
        let dir = TestDir::new().source_file(
            "simple.ts",
            "function add(a: number, b: number): number { return a + b }",
        );

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].target.contains("add"));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn scores_tsx_file() {
        let dir =
            TestDir::new().source_file("component.tsx", "function Greeting() { return 'hello' }");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].target.contains("Greeting"));
    }

    #[test]
    fn scores_mixed_language_project() {
        let dir = TestDir::new()
            .source_file("lib.rs", "fn rust_fn() { if true {} }")
            .source_file("app.ts", "function ts_fn() { return 1 }");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 2);
        let names: Vec<&str> = evals.iter().map(|e| e.target.as_str()).collect();
        assert!(names.iter().any(|t| t.contains("rust_fn")));
        assert!(names.iter().any(|t| t.contains("ts_fn")));
    }
}
