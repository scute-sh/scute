use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::language::{self, LanguageConfig};
use super::{CloneGroup, Occurrence, SourceEntry, TreeSitterParser, find_clones};
use crate::parser::AstParser;
use serde::Deserialize;

use crate::files;
use crate::{Evaluation, Evidence, ExecutionError, Thresholds};

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
    let min_tokens = definition.min_tokens.unwrap_or(DEFAULT_MIN_TOKENS);
    let thresholds = definition.thresholds.clone().unwrap_or(Thresholds {
        warn: Some(DEFAULT_WARN),
        fail: Some(DEFAULT_FAIL),
    });

    let canonical_dir = files::validate_source_dir(source_dir)?;
    let focus_files = match files::validate_focus_files(
        focus_files,
        &["rs", "js", "jsx", "mjs", "cjs", "ts", "tsx"],
        "only Rust, JavaScript, and TypeScript files are supported",
    ) {
        Ok(files) => files,
        Err(errors) => return Ok(errors),
    };

    let skip_ignored = definition.skip_ignored_files.unwrap_or(true);
    let exclude = definition.exclude.as_deref().unwrap_or_default();
    let sources = read_sources(&canonical_dir, skip_ignored, exclude);
    let clone_groups = detect_clones(&sources, min_tokens)?;
    let relevant = filter_by_focus(&clone_groups, &focus_files);

    if relevant.is_empty() {
        return Ok(vec![Evaluation::completed(
            source_dir.display().to_string(),
            0,
            thresholds,
            vec![],
        )]);
    }

    let test_thresholds = definition.test_thresholds.clone().unwrap_or(Thresholds {
        warn: Some(DEFAULT_TEST_WARN),
        fail: Some(DEFAULT_TEST_FAIL),
    });
    Ok(build_evaluations(
        &relevant,
        &sources,
        &thresholds,
        &test_thresholds,
    ))
}

fn filter_by_focus<'a>(
    clone_groups: &'a [CloneGroup],
    focus_files: &[PathBuf],
) -> Vec<&'a CloneGroup> {
    let focus_strings: Vec<String> = focus_files
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    clone_groups
        .iter()
        .filter(|group| {
            focus_strings.is_empty()
                || group
                    .occurrences
                    .iter()
                    .any(|occ| focus_strings.contains(&occ.source_id))
        })
        .collect()
}

fn read_sources(
    dir: &Path,
    skip_ignored: bool,
    exclude: &[String],
) -> Vec<(String, String, &'static LanguageConfig)> {
    discover_files(dir, skip_ignored, exclude)
        .into_iter()
        .filter_map(|(path, lang)| {
            let content = std::fs::read_to_string(&path).ok()?;
            Some((path.display().to_string(), content, lang))
        })
        .collect()
}

fn detect_clones(
    sources: &[(String, String, &'static LanguageConfig)],
    min_tokens: usize,
) -> Result<Vec<CloneGroup>, ExecutionError> {
    let entries: Vec<SourceEntry<'_>> = sources
        .iter()
        .map(|(path, content, lang)| SourceEntry::new(content, path, lang))
        .collect();
    find_clones(&entries, min_tokens).map_err(|e| ExecutionError {
        code: "detection_failed".into(),
        message: e.to_string(),
        recovery: "check that source files are valid".into(),
    })
}

fn build_evaluations(
    groups: &[&CloneGroup],
    sources: &[(String, String, &'static LanguageConfig)],
    thresholds: &Thresholds,
    test_thresholds: &Thresholds,
) -> Vec<Evaluation> {
    let mut parser = TreeSitterParser::new();
    let source_by_path: HashMap<&str, (&str, &'static LanguageConfig)> = sources
        .iter()
        .map(|(path, content, lang)| (path.as_str(), (content.as_str(), *lang)))
        .collect();
    groups
        .iter()
        .map(|group| {
            let effective = if is_test_only_group(&mut parser, group, &source_by_path) {
                test_thresholds
            } else {
                thresholds
            };
            to_evaluation(group, effective, &source_by_path)
        })
        .collect()
}

fn is_test_only_group(
    parser: &mut dyn AstParser,
    group: &CloneGroup,
    sources: &HashMap<&str, (&str, &'static LanguageConfig)>,
) -> bool {
    group.occurrences.iter().all(|occ| {
        sources
            .get(occ.source_id.as_str())
            .is_some_and(|(content, lang)| {
                lang.is_test_context(
                    parser,
                    Path::new(&occ.source_id),
                    content,
                    occ.start_line,
                    occ.end_line,
                )
            })
    })
}

fn discover_files(
    dir: &Path,
    skip_ignored: bool,
    exclude: &[String],
) -> Vec<(PathBuf, &'static LanguageConfig)> {
    let mut result: Vec<_> = files::walk_source_files(dir, skip_ignored, exclude)
        .filter_map(|e| {
            let lang = language_for_path(e.path())?;
            Some((e.into_path(), lang))
        })
        .collect();
    result.sort_by(|(a, _), (b, _)| a.cmp(b));
    result
}

fn language_for_path(path: &Path) -> Option<&'static LanguageConfig> {
    static RUST: std::sync::LazyLock<LanguageConfig> = std::sync::LazyLock::new(language::rust);
    static JAVASCRIPT: std::sync::LazyLock<LanguageConfig> =
        std::sync::LazyLock::new(language::javascript);
    static TYPESCRIPT: std::sync::LazyLock<LanguageConfig> =
        std::sync::LazyLock::new(language::typescript);
    static TYPESCRIPT_TSX: std::sync::LazyLock<LanguageConfig> =
        std::sync::LazyLock::new(language::typescript_tsx);

    match path.extension()?.to_str()? {
        "rs" => Some(&RUST),
        "js" | "jsx" | "mjs" | "cjs" => Some(&JAVASCRIPT),
        "ts" => Some(&TYPESCRIPT),
        "tsx" => Some(&TYPESCRIPT_TSX),
        _ => None,
    }
}

/// A line is "trivial" if it's only punctuation and whitespace (closing braces,
/// semicolons, etc.). We skip these when picking a representative snippet.
fn is_trivial_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.chars().all(|c| c.is_ascii_punctuation())
}

fn occurrence_evidence(
    occ: &Occurrence,
    token_count: usize,
    sources: &HashMap<&str, (&str, &'static LanguageConfig)>,
) -> Evidence {
    let line_count = occ.end_line.saturating_sub(occ.start_line) + 1;
    let snippet = sources
        .get(occ.source_id.as_str())
        .and_then(|(content, _)| {
            content
                .lines()
                .skip(occ.start_line.saturating_sub(1))
                .take(line_count)
                .map(str::trim)
                .find(|line| !is_trivial_line(line))
        });

    let found = match snippet {
        Some(line) => format!("{token_count} duplicated tokens, e.g. `{line}`"),
        None => format!("{token_count} duplicated tokens"),
    };

    Evidence {
        rule: None,
        location: Some(format!(
            "{}:{}-{}",
            occ.source_id, occ.start_line, occ.end_line
        )),
        found,
        expected: None,
    }
}

fn to_evaluation(
    group: &CloneGroup,
    thresholds: &Thresholds,
    sources: &HashMap<&str, (&str, &'static LanguageConfig)>,
) -> Evaluation {
    let evidence = group
        .occurrences
        .iter()
        .map(|occ| occurrence_evidence(occ, group.token_count, sources))
        .collect();

    let observed = u64::try_from(group.token_count).unwrap_or(u64::MAX);

    Evaluation::completed(
        group
            .occurrences
            .first()
            .map(|occ| format!("{}:{}", occ.source_id, occ.start_line))
            .unwrap_or_default(),
        observed,
        thresholds.clone(),
        evidence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Outcome;
    use googletest::prelude::*;
    use tempfile::TempDir;

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

    /// Create a temp directory with the given files and run a similarity check.
    /// Returns `(TempDir, Vec<Evaluation>)` — caller keeps `TempDir` alive for
    /// any assertions that reference paths.
    fn check_files(files: &[(&str, &str)]) -> (TempDir, Vec<Evaluation>) {
        let dir = make_dir(files);
        let evals = check_dir(dir.path());
        (dir, evals)
    }

    fn make_dir(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, content) in files {
            write_file(dir.path(), name, content);
        }
        dir
    }

    fn check_clone_pair() -> Vec<Evaluation> {
        check_files(CLONE_PAIR).1
    }

    fn check_clone_pair_with_thresholds(warn: u64, fail: u64) -> Vec<Evaluation> {
        let dir = make_dir(CLONE_PAIR);
        check(
            dir.path(),
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

    const CLONE_PAIR: &[(&str, &str)] = &[
        ("a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
        ("b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
    ];

    fn two_clone_pairs_dir() -> TempDir {
        let mut files = CLONE_PAIR.to_vec();
        files.extend_from_slice(&[
            ("c.rs", "const A: [i32; 5] = [10, 20, 30, 40, 50];"),
            ("d.rs", "const B: [u32; 5] = [60, 70, 80, 90, 100];"),
        ]);
        make_dir(&files)
    }

    fn write_file(dir: &Path, name: &str, content: &str) {
        if let Some(parent) = Path::new(name).parent() {
            std::fs::create_dir_all(dir.join(parent)).unwrap();
        }
        std::fs::write(dir.join(name), content).unwrap();
    }

    fn unwrap_evidence(eval: &Evaluation) -> &Vec<Evidence> {
        let Outcome::Completed { evidence, .. } = &eval.outcome else {
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
        let dir = TempDir::new().unwrap();

        let evals = check_dir(dir.path());

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
    fn observed_value_is_token_count_of_the_clone() {
        let evals = check_clone_pair();

        let Outcome::Completed { observed, .. } = &evals[0].outcome else {
            panic!("expected completed evaluation")
        };
        assert_that!(*observed, eq(14)); // fn $ID ( $ID : $ID ) -> $ID { $ID + $LIT } = 14 tokens
    }

    #[test]
    fn directory_with_only_unsupported_files_passes() {
        let (_, evals) = check_files(&[("readme.md", "# Hello"), ("data.json", "{}")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn discovers_files_in_subdirectories() {
        let (_, evals) = check_files(&[
            ("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
            ("lib/b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
        ]);

        assert_that!(evals, len(eq(1)));
        let evidence = unwrap_evidence(&evals[0]);
        assert_location_contains(evidence, "src");
        assert_location_contains(evidence, "lib");
    }

    fn gitignore_dir() -> TempDir {
        let dir = make_dir(&[
            (".gitignore", "vendor/\n"),
            ("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
            ("vendor/lib/b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
        ]);
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        dir
    }

    #[test]
    fn skips_gitignored_directories() {
        let dir = gitignore_dir();

        let evals = check_dir(dir.path());

        // vendor/ is gitignored → only src/a.rs discovered → no clone pair
        assert!(
            evals.iter().all(Evaluation::is_pass),
            "vendor/ should be excluded, got: {evals:?}"
        );
    }

    #[test]
    fn skip_ignored_files_false_scans_gitignored_directories() {
        let dir = gitignore_dir();

        let evals = check(
            dir.path(),
            &[],
            &Definition {
                skip_ignored_files: Some(false),
                ..low_threshold()
            },
        )
        .unwrap();

        // With skip_ignored_files disabled, vendor/ is scanned → clone pair found
        assert!(
            evals.iter().any(|e| !e.is_pass()),
            "vendor/ should be scanned when skip_ignored_files is false, got: {evals:?}"
        );
    }

    #[test]
    fn evidence_contains_all_occurrence_locations() {
        let evals = check_clone_pair();

        let evidence = unwrap_evidence(&evals[0]);
        assert_that!(evidence, len(eq(2)));
        assert_location_contains(evidence, "a.rs");
        assert_location_contains(evidence, "b.rs");
    }

    #[test]
    fn evidence_snippets_reflect_each_occurrence() {
        let evals = check_clone_pair();

        let evidence = unwrap_evidence(&evals[0]);
        assert_that!(evidence[0].found, contains_substring("fn foo"));
        assert_that!(evidence[1].found, contains_substring("fn bar"));
    }

    #[test]
    fn duplicated_code_returns_one_evaluation_per_clone_group() {
        let evals = check_clone_pair();

        assert_that!(evals, len(eq(1)));
    }

    #[test]
    fn nonexistent_target_returns_error() {
        let result = check(Path::new("/nonexistent/path"), &[], &Definition::default());

        let err = result.unwrap_err();
        assert_that!(err.code, eq("invalid_target"));
    }

    #[test]
    fn distinct_code_passes() {
        let (_, evals) = check_files(&[
            ("a.rs", "let x = 1 + 2;"),
            ("b.rs", "if true { return false; }"),
        ]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test_case::test_case(
        &[("a.ts", "function foo(x: number): number { return x + 1; }"),
          ("b.ts", "function bar(y: number): number { return y + 1; }")]
        ; "typescript"
    )]
    #[test_case::test_case(
        &[("a.js", "function foo(x) { return x + 1; }"),
          ("b.js", "function bar(y) { return y + 1; }")]
        ; "javascript"
    )]
    #[test_case::test_case(
        &[("a.jsx", "function Greeting({ name }) { return <div>Hello {name}</div>; }"),
          ("b.jsx", "function Welcome({ name }) { return <div>Hello {name}</div>; }")]
        ; "jsx"
    )]
    #[test_case::test_case(
        &[("a.js", "function foo(x) { return x + 1; }"),
          ("b.mjs", "function bar(y) { return y + 1; }")]
        ; "across js and mjs"
    )]
    #[test_case::test_case(
        &[("a.js", "function foo(x) { return x + 1; }"),
          ("b.cjs", "function bar(y) { return y + 1; }")]
        ; "across js and cjs"
    )]
    #[test_case::test_case(
        &[("a.tsx", "function Greeting({ name }: { name: string }) { return <div>Hello {name}</div>; }"),
          ("b.tsx", "function Welcome({ name }: { name: string }) { return <div>Hello {name}</div>; }")]
        ; "tsx"
    )]
    #[test_case::test_case(
        &[("a.ts", "function foo(x: number): number { return x + 1; }"),
          ("b.tsx", "function bar(y: number): number { return y + 1; }")]
        ; "across ts and tsx"
    )]
    fn detects_duplications(files: &[(&str, &str)]) {
        let (_, evals) = check_files(files);
        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_fail(), "expected fail, got: {evals:?}");
    }

    #[test]
    fn focus_file_only_reports_clone_groups_involving_that_file() {
        let dir = two_clone_pairs_dir();

        let evals = check_focused(dir.path(), &[dir.path().join("a.rs")]);

        assert_that!(evals, len(eq(1)));
        let evidence = unwrap_evidence(&evals[0]);
        assert_location_contains(evidence, "a.rs");
        assert_location_contains(evidence, "b.rs");
    }

    #[test]
    fn focus_file_without_clones_passes() {
        let dir = make_dir(&[
            ("clean.rs", "fn unique_stuff() -> bool { true }"),
            ("a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
            ("b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
        ]);

        let evals = check_focused(dir.path(), &[dir.path().join("clean.rs")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn multiple_focus_files_report_clones_involving_any_of_them() {
        let dir = two_clone_pairs_dir();

        let evals = check_focused(
            dir.path(),
            &[dir.path().join("a.rs"), dir.path().join("c.rs")],
        );

        assert_that!(evals, len(eq(2)));
    }

    #[test_case::test_case(
        &[("tests/a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
          ("tests/b.rs", "fn bar(y: i32) -> i32 { y + 1 }")]
        ; "test directory clones"
    )]
    #[test_case::test_case(
        &[("a.test.ts", "function foo(x: number): number { return x + 1; }"),
          ("b.test.ts", "function bar(y: number): number { return y + 1; }")]
        ; "typescript test files"
    )]
    #[test_case::test_case(
        &[("a.test.js", "function foo(x) { return x + 1; }"),
          ("b.test.js", "function bar(y) { return y + 1; }")]
        ; "javascript test files"
    )]
    #[test_case::test_case(
        &[("__tests__/a.js", "function foo(x) { return x + 1; }"),
          ("__tests__/b.js", "function bar(y) { return y + 1; }")]
        ; "js files in __tests__ directory"
    )]
    #[test_case::test_case(
        &[("a.spec.ts", "function foo(x: number): number { return x + 1; }"),
          ("b.spec.ts", "function bar(y: number): number { return y + 1; }")]
        ; "spec ts files"
    )]
    #[test_case::test_case(
        &[("a.test.tsx", "function Greeting({ name }: { name: string }) { return <div>Hello {name}</div>; }"),
          ("b.test.tsx", "function Welcome({ name }: { name: string }) { return <div>Hello {name}</div>; }")]
        ; "tsx test files"
    )]
    #[test_case::test_case(
        &[("src/a.rs", "#[test]\nfn test_a(x: i32) -> i32 { x + 1 }"),
          ("src/b.rs", "#[test]\nfn test_b(y: i32) -> i32 { y + 1 }")]
        ; "naked test fns"
    )]
    #[test_case::test_case(
        &[("src/a.rs", "fn serve() -> String { String::from(\"hello\") }\n\
                         #[cfg(test)]\nmod tests {\n    fn helper_a(x: i32) -> i32 { x + 1 }\n}"),
          ("src/b.rs", "use std::collections::HashMap;\n\
                         #[cfg(test)]\nmod tests {\n    fn helper_b(y: i32) -> i32 { y + 1 }\n}")]
        ; "inline rust test modules"
    )]
    fn applies_test_thresholds(files: &[(&str, &str)]) {
        let (_, evals) = check_files(files);
        assert!(
            evals[0].is_warn(),
            "expected warn (test thresholds), got: {evals:?}"
        );
    }

    #[test]
    fn uses_production_thresholds_for_mixed_test_and_production_clones() {
        let (_, evals) = check_files(&[
            ("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
            ("tests/b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
        ]);

        assert!(
            evals[0].is_fail(),
            "mixed groups should use production thresholds, got: {evals:?}"
        );
    }

    #[test]
    fn single_file_without_duplication_passes() {
        let (_, evals) = check_files(&[("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn excludes_files_matching_a_glob_pattern() {
        let dir = make_dir(CLONE_PAIR);

        let evals = check(
            dir.path(),
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
        let dir = make_dir(&[
            ("a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
            ("b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
            ("c.ts", "function baz(z: number): number { return z + 1; }"),
        ]);

        let evals = check(
            dir.path(),
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
        let dir = make_dir(&[
            ("src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }"),
            ("generated/b.rs", "fn bar(y: i32) -> i32 { y + 1 }"),
        ]);

        let evals = check(
            dir.path(),
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
        let dir = make_dir(CLONE_PAIR);

        // 14 tokens < default min_tokens of 50 → no clones detected → pass
        let evals = check(dir.path(), &[], &Definition::default()).unwrap();

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }
}
