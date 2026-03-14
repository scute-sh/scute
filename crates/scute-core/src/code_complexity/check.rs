use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::score;
use crate::files;
use crate::{Evaluation, Evidence, ExecutionError, Expected, Thresholds};

pub const CHECK_NAME: &str = "code-complexity";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Definition {
    pub thresholds: Option<Thresholds>,
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

/// # Errors
///
/// Returns `ExecutionError` if `source_dir` is not a valid directory.
pub fn check(
    source_dir: &Path,
    focus_files: &[PathBuf],
    definition: &Definition,
) -> Result<Vec<Evaluation>, ExecutionError> {
    let canonical_dir = files::validate_source_dir(source_dir)?;

    let thresholds = definition.thresholds();
    let exclude = definition.exclude.as_deref().unwrap_or_default();
    let rust_files = discover_rust_files(&canonical_dir, exclude);

    let focus =
        match files::validate_focus_files(focus_files, &["rs"], "only Rust files are supported") {
            Ok(files) => files,
            Err(errors) => return Ok(errors),
        };

    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut evaluations = Vec::new();

    let paths = rust_files
        .iter()
        .filter(|p| focus.is_empty() || focus.contains(p));

    for path in paths {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        evaluations.extend(score_file(path, &source, &language, &thresholds));
    }

    if evaluations.is_empty() {
        evaluations.push(Evaluation::completed(
            source_dir.display().to_string(),
            0,
            thresholds,
            vec![],
        ));
    }

    Ok(evaluations)
}

fn score_file(
    path: &Path,
    source: &str,
    language: &tree_sitter::Language,
    thresholds: &Thresholds,
) -> Vec<Evaluation> {
    score::score_functions(source, language)
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

fn format_nesting_chain(chain: &[score::Construct]) -> String {
    chain
        .iter()
        .map(|c| c.label())
        .collect::<Vec<_>>()
        .join(" > ")
}

fn pluralize_levels(n: u64) -> &'static str {
    if n == 1 { "level" } else { "levels" }
}

fn format_ops(operators: &[String]) -> String {
    let mut unique: Vec<&str> = operators.iter().map(String::as_str).collect();
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
                construct.label(),
                construct.flow_break_label(),
                c.increment
            ),
            None,
        ),
        score::ContributorKind::Nesting {
            construct,
            depth,
            chain,
        } => {
            let name = construct.label();
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

fn discover_rust_files(dir: &Path, exclude: &[String]) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = files::walk_source_files(dir, true, exclude)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(ignore::DirEntry::into_path)
        .collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use test_case::test_case;

    fn check_dir(dir: &Path) -> Vec<Evaluation> {
        check(dir, &[], &Definition::default()).unwrap()
    }

    #[test]
    fn returns_one_evaluation_per_function() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("two.rs"),
            "fn a() {} fn b(x: i32) -> i32 { if x > 0 { 1 } else { -1 } }",
        )
        .unwrap();

        let evals = check_dir(dir.path());

        assert_eq!(evals.len(), 2);
        assert!(evals[0].target.contains('a'));
        assert!(evals[1].target.contains('b'));
    }

    #[test]
    fn returns_single_pass_for_empty_directory() {
        let dir = tempfile::tempdir().unwrap();

        let evals = check_dir(dir.path());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass());
    }

    #[test]
    fn focus_files_limits_to_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.rs");
        fs::write(&target, "fn focused() { if true {} }").unwrap();
        fs::write(dir.path().join("other.rs"), "fn ignored() { if true {} }").unwrap();

        let evals = check(dir.path(), &[target], &Definition::default()).unwrap();

        assert!(evals.iter().all(|e| e.target.contains("focused")));
    }

    #[test]
    fn applies_default_thresholds() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("simple.rs"), "fn f() { if true {} }").unwrap();

        let evals = check_dir(dir.path());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass()); // score 1, default warn 5
    }

    fn evidence_of(source: &str) -> Vec<Evidence> {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), source).unwrap();

        let mut evals = check_dir(dir.path());
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
    fn rejects_nonexistent_source_dir() {
        let result = check(Path::new("/does/not/exist"), &[], &Definition::default());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "invalid_target");
    }

    #[test]
    fn skips_non_rust_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("code.py"), "def foo(): pass").unwrap();

        let evals = check_dir(dir.path());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass()); // fallback pass, no rust files
    }

    #[test]
    fn nonexistent_focus_file_produces_error() {
        let dir = tempfile::tempdir().unwrap();

        let evals = check(
            dir.path(),
            &[PathBuf::from("/nonexistent/file.rs")],
            &Definition::default(),
        )
        .unwrap();

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_error());
    }

    #[test]
    fn unsupported_focus_file_extension_produces_error() {
        let dir = tempfile::tempdir().unwrap();
        let py_file = dir.path().join("code.py");
        fs::write(&py_file, "def foo(): pass").unwrap();

        let evals = check(dir.path(), &[py_file], &Definition::default()).unwrap();

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_error());
    }
}
