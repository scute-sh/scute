use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::detect::{CloneGroup, detect_clones as run_detection};
use super::evaluate::{SourceContext, ThresholdSet, evaluate_groups};
use super::parse_source;
use super::rules::SimilarityRules;
use super::tree::SourceTree;
use crate::files;
use crate::files::SourceFile;
use crate::language::{self, JsFamily, LanguageRegistry, Rust};
use crate::{Evaluation, ExecutionError, Thresholds};

pub const CHECK_NAME: &str = "code-similarity";

const DEFAULT_MIN_TOKENS: usize = 50;
const DEFAULT_WARN: u64 = 70;
const DEFAULT_FAIL: u64 = 100;
const DEFAULT_TEST_WARN: u64 = 100;
const DEFAULT_TEST_FAIL: u64 = 130;

/// Configuration for the code similarity check.
///
/// All fields are optional and fall back to sensible defaults when absent.
///
/// ```
/// use scute_core::code_similarity::Definition;
///
/// // Zero-config: uses default min_tokens (50) and thresholds (warn: 70, fail: 100)
/// let default = Definition::default();
///
/// // Custom: catch smaller clones, tighter thresholds
/// let strict = Definition {
///     min_tokens: Some(10),
///     thresholds: Some(scute_core::Thresholds { warn: Some(15), fail: Some(30) }),
///     ..Definition::default()
/// };
/// ```
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// Minimum token count for a sequence to be considered a clone.
    /// Defaults to 50.
    #[serde(alias = "min-tokens")]
    pub min_tokens: Option<usize>,
    pub thresholds: Option<Thresholds>,
    /// Skip files matching `.gitignore`, `.ignore`, and hidden paths.
    /// Defaults to `true`.
    #[serde(alias = "skip-ignored-files")]
    pub skip_ignored_files: Option<bool>,
    /// Separate thresholds for clone groups where every occurrence lives
    /// in test code. Defaults to warn: 100, fail: 130.
    #[serde(alias = "test-thresholds")]
    pub test_thresholds: Option<Thresholds>,
    /// Glob patterns for files to exclude from similarity analysis.
    pub exclude: Option<Vec<String>>,
}

impl Definition {
    fn min_tokens(&self) -> usize {
        self.min_tokens.unwrap_or(DEFAULT_MIN_TOKENS)
    }

    fn thresholds(&self) -> ThresholdSet {
        ThresholdSet {
            base: self.thresholds.clone().unwrap_or(Thresholds {
                warn: Some(DEFAULT_WARN),
                fail: Some(DEFAULT_FAIL),
            }),
            test: self.test_thresholds.clone().unwrap_or(Thresholds {
                warn: Some(DEFAULT_TEST_WARN),
                fail: Some(DEFAULT_TEST_FAIL),
            }),
        }
    }

    fn skip_ignored(&self) -> bool {
        self.skip_ignored_files.unwrap_or(true)
    }

    fn exclude_patterns(&self) -> &[String] {
        self.exclude.as_deref().unwrap_or_default()
    }
}

/// Check a directory for code duplication.
///
/// Discovers supported source files (Rust, JavaScript, TypeScript), runs
/// clone detection, and returns one [`Evaluation`] per clone group found.
/// When no clones are detected, returns a single passing evaluation.
///
/// When `focus_files` is non-empty, only clone groups involving at least
/// one focus file are reported. An empty slice means full-project scan.
/// Focus files with unsupported extensions or that can't be read produce
/// errored evaluations.
///
/// ```no_run
/// use std::path::Path;
/// use scute_core::code_similarity::{Definition, check};
///
/// let evals = check(Path::new("."), &[], &Definition::default()).unwrap();
/// for eval in &evals {
///     if eval.is_fail() {
///         eprintln!("duplication found: {}", eval.target);
///     }
/// }
/// ```
///
/// # Errors
///
/// Returns `ExecutionError` if `source_dir` is not a valid directory.
pub fn check(
    source_dir: &Path,
    focus_files: &[PathBuf],
    definition: &Definition,
) -> Result<Vec<Evaluation>, ExecutionError> {
    let canonical_dir = files::validate_source_dir(source_dir).map_err(|e| ExecutionError {
        code: "invalid_target".into(),
        message: e.to_string(),
        recovery: "check that the path exists and is a directory".into(),
    })?;
    let languages = languages();
    let focus_files = match files::validate_focus_files(
        focus_files,
        &languages.supported_extensions(),
        "only Rust, JavaScript, and TypeScript files are supported",
    ) {
        Ok(files) => files,
        Err(errors) => return Ok(errors),
    };

    let source_files = read_sources(&canonical_dir, definition, &languages);
    let trees = parse_trees(&source_files, &languages)?;
    let token_sets: Vec<Vec<_>> = trees.iter().map(SourceTree::tokens).collect();
    let contexts = build_contexts(&trees, &token_sets, &source_files);

    let clone_groups = run_detection(&token_sets, definition.min_tokens());
    let relevant = filter_by_focus(&clone_groups, &focus_files, &contexts);
    let thresholds = definition.thresholds();
    let evaluations = evaluate_groups(&relevant, &contexts, &thresholds);

    if evaluations.is_empty() {
        return Ok(vec![Evaluation::completed(
            source_dir.display().to_string(),
            0,
            thresholds.base,
            vec![],
        )]);
    }

    Ok(evaluations)
}

fn build_contexts<'a>(
    trees: &'a [SourceTree],
    token_sets: &'a [Vec<super::tree::Token>],
    sources: &'a [SourceFile],
) -> Vec<SourceContext<'a>> {
    trees
        .iter()
        .zip(token_sets)
        .zip(sources)
        .map(|((tree, tokens), source)| SourceContext {
            tree,
            tokens,
            content: &source.content,
        })
        .collect()
}

fn filter_by_focus<'a>(
    clone_groups: &'a [CloneGroup],
    focus_files: &[PathBuf],
    contexts: &[SourceContext],
) -> Vec<&'a CloneGroup> {
    let focus_strings: Vec<String> = focus_files
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    clone_groups
        .iter()
        .filter(|group| {
            focus_strings.is_empty()
                || group.occurrences.iter().any(|occ| {
                    focus_strings.contains(&contexts[occ.source_idx].tree.source_id().to_string())
                })
        })
        .collect()
}

fn read_sources(
    dir: &Path,
    definition: &Definition,
    languages: &LanguageRegistry<dyn SimilarityRules>,
) -> Vec<SourceFile> {
    let mut result: Vec<SourceFile> = files::walk_source_files(
        dir,
        definition.skip_ignored(),
        definition.exclude_patterns(),
    )
    .filter_map(|entry| {
        languages.for_path(entry.path())?;
        let content = std::fs::read_to_string(entry.path()).ok()?;
        Some(SourceFile {
            path: entry.into_path(),
            content,
        })
    })
    .collect();
    result.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

fn parse_trees(
    sources: &[SourceFile],
    languages: &LanguageRegistry<dyn SimilarityRules>,
) -> Result<Vec<SourceTree>, ExecutionError> {
    sources
        .iter()
        .map(|source| {
            parse_source(source, languages).map_err(|e| ExecutionError {
                code: "detection_failed".into(),
                message: e.to_string(),
                recovery: "check that source files are valid".into(),
            })
        })
        .collect()
}

#[must_use]
pub fn languages() -> LanguageRegistry<dyn SimilarityRules> {
    use crate::language::{LanguageRegistry, LanguageRegistryEntry};
    LanguageRegistry::new(vec![
        LanguageRegistryEntry {
            language: language::rust(),
            rules: Box::new(Rust),
        },
        LanguageRegistryEntry {
            language: language::javascript(),
            rules: Box::new(JsFamily),
        },
        LanguageRegistryEntry {
            language: language::typescript(),
            rules: Box::new(JsFamily),
        },
        LanguageRegistryEntry {
            language: language::typescript_tsx(),
            rules: Box::new(JsFamily),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Evidence;
    use googletest::prelude::*;
    use scute_test_utils::TestDir;

    fn low_threshold() -> Definition {
        Definition {
            min_tokens: Some(5),
            thresholds: Some(Thresholds {
                warn: Some(5),
                fail: Some(10),
            }),
            test_thresholds: Some(Thresholds {
                warn: Some(10),
                fail: Some(30),
            }),
            ..Definition::default()
        }
    }

    fn check_dir(dir: &Path) -> Vec<Evaluation> {
        check(dir, &[], &low_threshold()).unwrap()
    }

    fn check_focused(dir: &Path, focus_files: &[PathBuf]) -> Vec<Evaluation> {
        check(dir, focus_files, &low_threshold()).unwrap()
    }

    fn clone_pair() -> TestDir {
        TestDir::new()
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
    }

    fn check_clone_pair_with_thresholds(warn: u64, fail: u64) -> Vec<Evaluation> {
        let dir = clone_pair();
        check(
            &dir.root(),
            &[],
            &Definition {
                min_tokens: Some(5),
                thresholds: Some(Thresholds {
                    warn: Some(warn),
                    fail: Some(fail),
                }),
                ..Definition::default()
            },
        )
        .unwrap()
    }

    fn two_clone_pairs() -> TestDir {
        TestDir::new()
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
            .source_file("c.rs", "const A: [i32; 5] = [10, 20, 30, 40, 50];")
            .source_file("d.rs", "const B: [u32; 5] = [60, 70, 80, 90, 100];")
    }

    fn unwrap_evidence(eval: &Evaluation) -> &Vec<Evidence> {
        let crate::Outcome::Completed { evidence, .. } = &eval.outcome else {
            panic!("expected completed evaluation")
        };
        evidence
    }

    fn assert_location_contains(evidence: &[Evidence], substring: &str) {
        assert_that!(
            evidence,
            contains(matches_pattern!(Evidence {
                location: some(contains_substring(substring)),
                ..
            }))
        );
    }

    #[test]
    fn empty_directory_passes_with_zero_observed() {
        let dir = TestDir::new();

        let evals = check_dir(&dir.root());

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn clone_exceeding_fail_threshold_produces_fail_status() {
        let evals = check_clone_pair_with_thresholds(10, 12);

        assert!(evals[0].is_fail()); // 14 tokens > fail threshold of 12
    }

    #[test]
    fn clone_below_thresholds_produces_pass_status() {
        let evals = check_clone_pair_with_thresholds(20, 30);

        assert!(evals[0].is_pass()); // 14 tokens < warn threshold of 20
    }

    #[test]
    fn directory_with_only_unsupported_files_passes() {
        let dir = TestDir::new()
            .source_file("readme.md", "# Hello")
            .source_file("data.json", "{}");

        let evals = check_dir(&dir.root());

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn discovers_files_in_subdirectories() {
        let dir = TestDir::new()
            .source_file("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("lib/b.rs", "fn bar(y: i32) -> i32 { y + 1 }");

        let evals = check_dir(&dir.root());

        assert_that!(evals, len(eq(1)));
        let evidence = unwrap_evidence(&evals[0]);
        assert_location_contains(evidence, "src");
        assert_location_contains(evidence, "lib");
    }

    fn gitignore_dir() -> TestDir {
        TestDir::new()
            .source_file(".git/HEAD", "")
            .source_file(".gitignore", "vendor/\n")
            .source_file("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("vendor/lib/b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
    }

    #[test]
    fn skips_gitignored_directories() {
        let dir = gitignore_dir();

        let evals = check_dir(&dir.root());

        assert!(
            evals.iter().all(Evaluation::is_pass),
            "vendor/ should be excluded, got: {evals:?}"
        );
    }

    #[test]
    fn skip_ignored_files_false_scans_gitignored_directories() {
        let dir = gitignore_dir();

        let evals = check(
            &dir.root(),
            &[],
            &Definition {
                skip_ignored_files: Some(false),
                ..low_threshold()
            },
        )
        .unwrap();

        assert!(
            evals.iter().any(|e| !e.is_pass()),
            "vendor/ should be scanned when skip_ignored_files is false, got: {evals:?}"
        );
    }

    #[test]
    fn duplicated_code_returns_one_evaluation_per_clone_group() {
        let dir = clone_pair();

        let evals = check_dir(&dir.root());

        assert_that!(evals, len(eq(1)));
    }

    #[test]
    fn nonexistent_target_returns_error() {
        let result = check(Path::new("/nonexistent/path"), &[], &Definition::default());

        let err = result.unwrap_err();
        assert_that!(err.code, eq("invalid_target"));
    }

    #[test_case::test_case("a.ts", "b.ts",
        "function foo(x: number): number { return x + 1; }",
        "function bar(y: number): number { return y + 1; }"
        ; "typescript"
    )]
    #[test_case::test_case("a.js", "b.js",
        "function foo(x) { return x + 1; }",
        "function bar(y) { return y + 1; }"
        ; "javascript"
    )]
    #[test_case::test_case("a.jsx", "b.jsx",
        "function Greeting({ name }) { return <div>Hello {name}</div>; }",
        "function Welcome({ name }) { return <div>Hello {name}</div>; }"
        ; "jsx"
    )]
    #[test_case::test_case("a.js", "b.mjs",
        "function foo(x) { return x + 1; }",
        "function bar(y) { return y + 1; }"
        ; "across js and mjs"
    )]
    #[test_case::test_case("a.js", "b.cjs",
        "function foo(x) { return x + 1; }",
        "function bar(y) { return y + 1; }"
        ; "across js and cjs"
    )]
    #[test_case::test_case("a.tsx", "b.tsx",
        "function Greeting({ name }: { name: string }) { return <div>Hello {name}</div>; }",
        "function Welcome({ name }: { name: string }) { return <div>Hello {name}</div>; }"
        ; "tsx"
    )]
    #[test_case::test_case("a.ts", "b.tsx",
        "function foo(x: number): number { return x + 1; }",
        "function bar(y: number): number { return y + 1; }"
        ; "across ts and tsx"
    )]
    fn detects_duplications(file_a: &str, file_b: &str, content_a: &str, content_b: &str) {
        let dir = TestDir::new()
            .source_file(file_a, content_a)
            .source_file(file_b, content_b);

        let evals = check_dir(&dir.root());

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_fail(), "expected fail, got: {evals:?}");
    }

    #[test]
    fn focus_file_only_reports_clone_groups_involving_that_file() {
        let dir = two_clone_pairs();

        let evals = check_focused(&dir.root(), &[dir.path("a.rs")]);

        assert_that!(evals, len(eq(1)));
        let evidence = unwrap_evidence(&evals[0]);
        assert_location_contains(evidence, "a.rs");
        assert_location_contains(evidence, "b.rs");
    }

    #[test]
    fn focus_file_without_clones_passes() {
        let dir = TestDir::new()
            .source_file("clean.rs", "fn unique_stuff() -> bool { true }")
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }");

        let evals = check_focused(&dir.root(), &[dir.path("clean.rs")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn multiple_focus_files_report_clones_involving_any_of_them() {
        let dir = two_clone_pairs();

        let evals = check_focused(&dir.root(), &[dir.path("a.rs"), dir.path("c.rs")]);

        assert_that!(evals, len(eq(2)));
    }

    #[test]
    fn excludes_files_matching_a_glob_pattern() {
        let dir = clone_pair();

        let evals = check(
            &dir.root(),
            &[],
            &Definition {
                exclude: Some(vec!["b.rs".to_string()]),
                ..low_threshold()
            },
        )
        .unwrap();

        assert!(
            evals.iter().all(Evaluation::is_pass),
            "b.rs should be excluded, got: {evals:?}"
        );
    }

    #[test]
    fn excludes_files_matching_multiple_glob_patterns() {
        let dir = TestDir::new()
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
            .source_file("c.ts", "function baz(z: number): number { return z + 1; }");

        let evals = check(
            &dir.root(),
            &[],
            &Definition {
                exclude: Some(vec!["b.rs".to_string(), "*.ts".to_string()]),
                ..low_threshold()
            },
        )
        .unwrap();

        assert!(
            evals.iter().all(Evaluation::is_pass),
            "b.rs and *.ts should be excluded, got: {evals:?}"
        );
    }

    #[test]
    fn excludes_files_in_subdirectory_matching_glob_pattern() {
        let dir = TestDir::new()
            .source_file("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("generated/b.rs", "fn bar(y: i32) -> i32 { y + 1 }");

        let evals = check(
            &dir.root(),
            &[],
            &Definition {
                exclude: Some(vec!["generated/**".to_string()]),
                ..low_threshold()
            },
        )
        .unwrap();

        assert!(
            evals.iter().all(Evaluation::is_pass),
            "generated/** should be excluded, got: {evals:?}"
        );
    }

    #[test]
    fn default_definition_uses_sensible_defaults() {
        let dir = clone_pair();

        // 14 tokens < default min_tokens of 50 → no clones detected → pass
        let evals = check(&dir.root(), &[], &Definition::default()).unwrap();

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }
}
