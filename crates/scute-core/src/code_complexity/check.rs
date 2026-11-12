use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::rules::{self, ComplexityRules};
use super::score;
use crate::files;
use crate::files::{Exclusions, SourceFile};
use crate::language::{self, Java, JsFamily, LanguageRegistry, Rust};
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
/// discover supported files. Without paths (`None`), the whole
/// `project_root` is scored. Files matching `exclude` patterns, relative to
/// `project_root`, are skipped whether found in a directory or given directly.
///
/// Returns one [`Evaluation`] per function found, and none when there's no
/// function to score (no supported files, or files without functions). A
/// path that doesn't exist or can't be reached gets its own errored evaluation,
/// and the other paths are still scored.
///
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use scute_core::code_complexity::{self, Definition};
///
/// let evals = code_complexity::check(
///     Path::new("."),
///     Some(&[PathBuf::from("src/")]),
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
/// Doesn't fail as a whole: problems with individual paths are reported as
/// errored evaluations instead.
pub fn check(
    project_root: &Path,
    paths: Option<&[PathBuf]>,
    definition: &Definition,
) -> Result<Vec<Evaluation>, ExecutionError> {
    let whole_project = [project_root.to_path_buf()];
    let paths = paths.unwrap_or(&whole_project);
    let thresholds = definition.thresholds();
    let languages = languages();
    let resolved = resolve_files(project_root, paths, definition, &languages);

    let errors = resolved.invalid.into_iter().map(Evaluation::from);
    let scores = resolved
        .files
        .iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).ok()?;
            let file = SourceFile {
                path: path.clone(),
                content,
            };
            Some(score_file(&file, &languages, &thresholds))
        })
        .flatten();
    Ok(errors.chain(scores).collect())
}

fn resolve_files(
    project_root: &Path,
    paths: &[PathBuf],
    definition: &Definition,
    languages: &LanguageRegistry<dyn ComplexityRules>,
) -> files::ResolvedPaths {
    let exclusions = Exclusions::new(
        project_root,
        definition.exclude.as_deref().unwrap_or_default(),
    );
    files::resolve_paths(paths, &languages.supported_extensions(), &exclusions)
}

/// The languages this check scores, each with its scoring rules.
#[must_use]
pub fn languages() -> LanguageRegistry<dyn ComplexityRules> {
    LanguageRegistry::<dyn ComplexityRules>::default()
        .with(Box::new(Rust), language::rust_family())
        .with(Box::new(JsFamily), language::js_family())
        .with(Box::new(Java), language::java_family())
}

fn score_file(
    file: &SourceFile,
    languages: &LanguageRegistry<dyn ComplexityRules>,
    thresholds: &Thresholds,
) -> Vec<Evaluation> {
    let path = Path::new(&file.path);
    score::score_functions(file, languages)
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

fn format_nesting_chain(chain: &[rules::FlowConstruct]) -> String {
    chain
        .iter()
        .map(|c| c.label)
        .collect::<Vec<_>>()
        .join(" > ")
}

fn pluralize_levels(n: u64) -> &'static str {
    if n == 1 { "level" } else { "levels" }
}

fn format_operators(operators: &[rules::LogicalOp]) -> String {
    let mut unique: Vec<&str> = operators.iter().map(|o| o.label()).collect();
    unique.sort_unstable();
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
        score::ContributorKind::Else { label } => (
            "else",
            format!("'{label}' branch (+{})", c.increment),
            text("use a guard clause or early return"),
        ),
        score::ContributorKind::Logical { operators } => (
            "boolean logic",
            format!(
                "{} operators (+{})",
                format_operators(operators),
                c.increment
            ),
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
    use googletest::prelude::*;
    use scute_test_utils::TestDir;
    use test_case::test_case;

    fn check_dir(dir: &Path) -> Vec<Evaluation> {
        check(dir, None, &Definition::default()).unwrap()
    }

    #[test]
    fn returns_one_evaluation_per_function() {
        let dir = TestDir::new().source_file(
            "two.rs",
            "fn a() {} fn b(x: i32) -> i32 { if x > 0 { 1 } else { -1 } }",
        );

        let evals = check_dir(&dir.root());

        assert_that!(
            evals,
            elements_are![
                field!(Evaluation.target, ends_with(":1:a")),
                field!(Evaluation.target, ends_with(":1:b")),
            ]
        );
    }

    // for: +1, if: +1+1, two else ifs: +1 each = 5, the default warn threshold
    #[test]
    fn nested_else_if_chain_at_warn_threshold_passes() {
        let dir = TestDir::new().source_file(
            "chain.rs",
            "fn f(a: bool, c: &[i32]) { for _ in c { if a {} else if a {} else if a {} } }",
        );

        let evals = check_dir(&dir.root());

        assert!(evals[0].is_pass());
    }

    #[test]
    fn empty_directory_returns_no_evaluations() {
        let dir = TestDir::new();

        let evals = check_dir(&dir.root());

        assert!(evals.is_empty(), "got: {evals:?}");
    }

    #[test]
    fn scores_only_functions_in_specified_file() {
        let dir = TestDir::new()
            .source_file("target.rs", "fn focused() { if true {} }")
            .source_file("other.rs", "fn ignored() { if true {} }");

        let evals = check(
            &dir.root(),
            Some(&[dir.path("target.rs")]),
            &Definition::default(),
        )
        .unwrap();

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
        evidence_of_file("a.rs", source)
    }

    fn evidence_of_file(filename: &str, source: &str) -> Vec<Evidence> {
        let dir = TestDir::new().source_file(filename, source);

        let mut evals = check_dir(&dir.root());
        let crate::Outcome::Completed { evidence, .. } = evals.remove(0).outcome else {
            panic!("expected completed");
        };
        evidence
    }

    #[test_case(
        "a.rs", "fn f() { if true {} }",
        "flow break", "'if' conditional (+1)", None
        ; "flow_break_has_no_suggestion"
    )]
    #[test_case(
        "a.rs", "fn f() { for x in [1] { if true {} } }",
        "nesting", "'if' nested 1 level: 'for > if' (+2)", Some("extract inner block into a function")
        ; "nesting_shows_chain_and_suggests_extraction"
    )]
    #[test_case(
        "a.rs", "fn f(x: bool) { if x {} else {} }",
        "else", "'else' branch (+1)", Some("use a guard clause or early return")
        ; "else_suggests_guard_clause"
    )]
    #[test_case(
        "a.rs", "fn f(a: bool, b: bool) { if a {} else if b {} }",
        "else", "'else if' branch (+1)", Some("use a guard clause or early return")
        ; "else_if_names_the_branch"
    )]
    #[test_case(
        "a.rs", "fn f(a: bool, b: bool, c: &[i32]) { for _ in c { if a {} else if b {} } }",
        "else", "'else if' branch (+1)", Some("use a guard clause or early return")
        ; "nested_else_if_reports_no_nesting"
    )]
    #[test_case(
        "a.rs", "fn f(a: bool, b: bool, c: bool) { if a {} else if b { if c {} } }",
        "nesting", "'if' nested 1 level: 'else if > if' (+2)", Some("extract inner block into a function")
        ; "nesting_chain_counts_else_if_as_one_level"
    )]
    #[test_case(
        "a.rs", "fn f(a: bool, b: bool, c: bool) { if a {} else if b {} else if c { if a {} } }",
        "nesting", "'if' nested 1 level: 'else if > if' (+2)", Some("extract inner block into a function")
        ; "nesting_chain_skips_sibling_branches_of_else_if_chain"
    )]
    #[test_case(
        "a.rs", "fn f(a: bool, b: bool) -> bool { a && b }",
        "boolean logic", "'&&' operators (+1)", Some("extract into a named boolean")
        ; "logical_single_operator"
    )]
    #[test_case(
        "a.rs", "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }",
        "boolean logic", "mixed '&&' and '||' operators (+2)", Some("extract into a named boolean")
        ; "logical_mixed_operators"
    )]
    #[test_case(
        "a.rs", "fn go(n: u64) -> u64 { go(n - 1) }",
        "recursion", "recursive call to 'go' (+1)", Some("consider iterative approach")
        ; "recursion_shows_function_name"
    )]
    #[test_case(
        "a.rs", "fn f() { 'outer: loop { break 'outer; } }",
        "jump", "'break' to label 'outer (+1)", Some("restructure to avoid labeled jump")
        ; "jump_shows_label"
    )]
    #[test_case(
        "A.java", "class A { void f(List<Integer> c) { for (int x : c) {} } }",
        "flow break", "'for-each' loop (+1)", None
        ; "java_for_each_names_the_loop"
    )]
    #[test_case(
        "A.java", "class A { void f(List<Integer> c) { for (int x : c) { if (x > 0) {} } } }",
        "nesting", "'if' nested 1 level: 'for-each > if' (+2)", Some("extract inner block into a function")
        ; "java_nesting_chain_names_the_for_each"
    )]
    #[test_case(
        "A.java", "class A { void f(boolean a, boolean b, boolean c) { if (a) {} else if (b) {} else if (c) { if (a) {} } } }",
        "nesting", "'if' nested 1 level: 'else if > if' (+2)", Some("extract inner block into a function")
        ; "java_nesting_chain_counts_else_if_as_one_level"
    )]
    fn evidence_formatting(
        filename: &str,
        source: &str,
        rule: &str,
        expected_found: &str,
        expected: Option<&str>,
    ) {
        let evidence = evidence_of_file(filename, source);
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

    #[test_case("a.ts" ; "typescript")]
    #[test_case("a.js" ; "javascript")]
    fn evidence_names_catch_as_exception_handler(filename: &str) {
        let evidence = evidence_of_file(filename, "function f() { try {} catch (e) {} }");
        assert_that!(
            evidence,
            elements_are![matches_pattern!(Evidence {
                rule: some(eq("flow break")),
                found: eq("'catch' exception handler (+1)"),
                ..
            })]
        );
    }

    #[test]
    fn evidence_includes_file_location() {
        let evidence = evidence_of("fn f() { if true {} }");

        assert!(evidence[0].location.as_ref().unwrap().contains("a.rs:1"));
    }

    #[test_case::test_case("/does/not/exist" ; "without extension")]
    #[test_case::test_case("/nonexistent/file.rs" ; "source file")]
    fn nonexistent_path_returns_an_error_naming_it(path: &str) {
        let evals = check(
            Path::new("."),
            Some(&[PathBuf::from(path)]),
            &Definition::default(),
        )
        .unwrap();

        assert_eq!(evals.len(), 1, "got: {evals:?}");
        assert!(evals[0].is_error());
        assert_eq!(evals[0].target, path);
    }

    #[test]
    fn directory_with_only_unsupported_files_returns_no_evaluations() {
        let dir = TestDir::new().source_file("code.py", "def foo(): pass");

        let evals = check_dir(&dir.root());

        assert!(evals.is_empty(), "got: {evals:?}");
    }

    #[test]
    fn skips_file_in_unsupported_language() {
        let dir = TestDir::new().source_file("code.py", "def foo(): pass");

        let evals = check(
            &dir.root(),
            Some(&[dir.path("code.py")]),
            &Definition::default(),
        )
        .unwrap();

        assert!(evals.is_empty(), "got: {evals:?}");
    }

    #[test]
    fn scores_tsx_file() {
        let dir =
            TestDir::new().source_file("component.tsx", "function Greeting() { return 'hello' }");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].target.contains("Greeting"));
    }

    #[test_case("app.js" ; "js")]
    #[test_case("app.jsx" ; "jsx")]
    #[test_case("app.mjs" ; "mjs")]
    #[test_case("app.cjs" ; "cjs")]
    fn scores_javascript_file(filename: &str) {
        let dir = TestDir::new().source_file(filename, "function greet() { return 'hi' }");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].target.contains("greet"));
    }

    #[test]
    fn scores_mixed_language_project() {
        let dir = TestDir::new()
            .source_file("lib.rs", "fn rust_fn() { if true {} }")
            .source_file("app.ts", "function ts_fn() { return 1 }")
            .source_file("util.js", "function js_fn() { return 2 }");

        let evals = check_dir(&dir.root());

        assert_eq!(evals.len(), 3);
        let names: Vec<&str> = evals.iter().map(|e| e.target.as_str()).collect();
        assert!(names.iter().any(|t| t.contains("rust_fn")));
        assert!(names.iter().any(|t| t.contains("ts_fn")));
        assert!(names.iter().any(|t| t.contains("js_fn")));
    }
}
