use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::language::{self, LanguageConfig};
use super::{CloneGroup, SourceEntry, find_clones};
use crate::{Evaluation, Evidence, ExecutionError, Outcome, Thresholds};

pub const CHECK_NAME: &str = "code-similarity";

const DEFAULT_MIN_TOKENS: usize = 25;
const DEFAULT_WARN: u64 = 40;
const DEFAULT_FAIL: u64 = 80;

/// Configuration for the code similarity check.
///
/// All fields are optional and fall back to sensible defaults when absent.
///
/// ```
/// use scute_core::code_similarity::Definition;
///
/// // Zero-config: uses default min_tokens (25) and thresholds (warn: 40, fail: 80)
/// let default = Definition::default();
///
/// // Custom: catch smaller clones, tighter thresholds
/// let strict = Definition {
///     min_tokens: Some(10),
///     thresholds: Some(scute_core::Thresholds { warn: Some(15), fail: Some(30) }),
/// };
/// ```
#[derive(Debug, Default)]
pub struct Definition {
    /// Minimum token count for a sequence to be considered a clone.
    /// Defaults to 25.
    pub min_tokens: Option<usize>,
    pub thresholds: Option<Thresholds>,
}

/// Check a directory for code duplication.
///
/// Discovers supported source files (`.rs`, `.ts`, `.tsx`), runs clone
/// detection, and returns one [`Evaluation`] per clone group found.
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

    let focus_errors = validate_focus_files(focus_files);
    if !focus_errors.is_empty() {
        return Ok(focus_errors);
    }

    let sources = read_sources(source_dir)?;

    let entries: Vec<SourceEntry<'_>> = sources
        .iter()
        .map(|(path, content, lang)| SourceEntry::new(content, path, lang))
        .collect();

    let clone_groups = find_clones(&entries, min_tokens).map_err(|e| ExecutionError {
        code: "detection_failed".into(),
        message: e.to_string(),
        recovery: "check that source files are valid".into(),
    })?;

    let relevant_groups = filter_by_focus(&clone_groups, focus_files);

    if relevant_groups.is_empty() {
        return Ok(vec![Evaluation {
            target: source_dir.display().to_string(),
            outcome: Outcome::completed(0, thresholds, vec![]),
        }]);
    }

    let content_by_path: HashMap<&str, &str> = sources
        .iter()
        .map(|(path, content, _)| (path.as_str(), content.as_str()))
        .collect();

    Ok(relevant_groups
        .iter()
        .map(|group| to_evaluation(group, &thresholds, &content_by_path))
        .collect())
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
) -> Result<Vec<(String, String, &'static LanguageConfig)>, ExecutionError> {
    let files = discover_files(dir)?;
    Ok(files
        .into_iter()
        .filter_map(|(path, lang)| {
            let content = std::fs::read_to_string(&path).ok()?;
            Some((path.display().to_string(), content, lang))
        })
        .collect())
}

fn validate_focus_files(focus_files: &[PathBuf]) -> Vec<Evaluation> {
    let mut errors = Vec::new();
    for path in focus_files {
        if language_for_path(path).is_none() {
            errors.push(Evaluation {
                target: path.display().to_string(),
                outcome: Outcome::Errored(ExecutionError {
                    code: "unsupported_language".into(),
                    message: format!("unsupported file type: {}", path.display()),
                    recovery: "only .rs, .ts, and .tsx files are supported".into(),
                }),
            });
        } else if !path.exists() {
            errors.push(Evaluation {
                target: path.display().to_string(),
                outcome: Outcome::Errored(ExecutionError {
                    code: "unreadable_file".into(),
                    message: format!("cannot read file: {}", path.display()),
                    recovery: "check that the file exists and is readable".into(),
                }),
            });
        }
    }
    errors
}

fn discover_files(dir: &Path) -> Result<Vec<(PathBuf, &'static LanguageConfig)>, ExecutionError> {
    let mut files = Vec::new();
    visit_dir(dir, &mut files).map_err(|e| ExecutionError {
        code: "invalid_target".into(),
        message: format!("cannot read directory {}: {e}", dir.display()),
        recovery: "check that the path exists and is a directory".into(),
    })?;
    files.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(files)
}

fn visit_dir(
    dir: &Path,
    files: &mut Vec<(PathBuf, &'static LanguageConfig)>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, files)?;
        } else if let Some(lang) = language_for_path(&path) {
            files.push((path, lang));
        }
    }
    Ok(())
}

fn language_for_path(path: &Path) -> Option<&'static LanguageConfig> {
    static RUST: std::sync::LazyLock<LanguageConfig> = std::sync::LazyLock::new(language::rust);
    static TYPESCRIPT: std::sync::LazyLock<LanguageConfig> =
        std::sync::LazyLock::new(language::typescript);

    match path.extension()?.to_str()? {
        "rs" => Some(&RUST),
        "ts" | "tsx" => Some(&TYPESCRIPT),
        _ => None,
    }
}

fn to_evaluation(
    group: &CloneGroup,
    thresholds: &Thresholds,
    content_by_path: &HashMap<&str, &str>,
) -> Evaluation {
    let snippet = group.occurrences.first().and_then(|occ| {
        let content = content_by_path.get(occ.source_id.as_str())?;
        content
            .lines()
            .skip(occ.start_line - 1)
            .take(occ.end_line - occ.start_line + 1)
            .map(str::trim)
            .find(|line| line.len() > 15)
    });

    let found = match snippet {
        Some(line) => format!("{} duplicated tokens, e.g. `{line}`", group.token_count),
        None => format!("{} duplicated tokens", group.token_count),
    };

    let evidence = group
        .occurrences
        .iter()
        .map(|occ| Evidence {
            rule: None,
            location: Some(format!(
                "{}:{}-{}",
                occ.source_id, occ.start_line, occ.end_line
            )),
            found: found.clone(),
            expected: None,
        })
        .collect();

    let observed = u64::try_from(group.token_count).unwrap_or(u64::MAX);

    Evaluation {
        target: group
            .occurrences
            .first()
            .map(|occ| format!("{}:{}", occ.source_id, occ.start_line))
            .unwrap_or_default(),
        outcome: Outcome::completed(observed, thresholds.clone(), evidence),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use tempfile::TempDir;

    const LOW_THRESHOLD: Definition = Definition {
        min_tokens: Some(5),
        thresholds: None,
    };

    fn check_dir(dir: &Path) -> Vec<Evaluation> {
        check(dir, &[], &LOW_THRESHOLD).unwrap()
    }

    fn check_focused(dir: &Path, focus_files: &[PathBuf]) -> Vec<Evaluation> {
        check(dir, focus_files, &LOW_THRESHOLD).unwrap()
    }

    fn clone_pair_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "a.rs", "fn foo(x: i32) -> i32 { x + 1 }");
        write_file(dir.path(), "b.rs", "fn bar(y: i32) -> i32 { y + 1 }");
        dir
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
        let dir = clone_pair_dir();

        let evals = check(
            dir.path(),
            &[],
            &Definition {
                min_tokens: Some(5),
                thresholds: Some(Thresholds {
                    warn: Some(10),
                    fail: Some(12),
                }),
            },
        )
        .unwrap();

        assert!(evals[0].is_fail()); // 14 tokens > fail threshold of 12
    }

    #[test]
    fn clone_below_thresholds_produces_pass_status() {
        let dir = clone_pair_dir();

        let evals = check(
            dir.path(),
            &[],
            &Definition {
                min_tokens: Some(5),
                thresholds: Some(Thresholds {
                    warn: Some(20),
                    fail: Some(30),
                }),
            },
        )
        .unwrap();

        assert!(evals[0].is_pass()); // 14 tokens < warn threshold of 20
    }

    #[test]
    fn observed_value_is_token_count_of_the_clone() {
        let dir = clone_pair_dir();

        let evals = check_dir(dir.path());

        let Outcome::Completed { observed, .. } = &evals[0].outcome else {
            panic!("expected completed evaluation")
        };
        assert_that!(*observed, eq(14)); // fn $ID ( $ID : $ID ) -> $ID { $ID + $LIT } = 14 tokens
    }

    #[test]
    fn directory_with_only_unsupported_files_passes() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "readme.md", "# Hello");
        write_file(dir.path(), "data.json", "{}");

        let evals = check_dir(dir.path());

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn discovers_files_in_subdirectories() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "src/a.rs", "fn foo(x: i32) -> i32 { x + 1 }");
        write_file(dir.path(), "lib/b.rs", "fn bar(y: i32) -> i32 { y + 1 }");

        let evals = check_dir(dir.path());

        assert_that!(evals, len(eq(1)));
        let evidence = unwrap_evidence(&evals[0]);
        assert_location_contains(evidence, "src");
        assert_location_contains(evidence, "lib");
    }

    #[test]
    fn evidence_contains_all_occurrence_locations() {
        let dir = clone_pair_dir();

        let evals = check_dir(dir.path());

        let evidence = unwrap_evidence(&evals[0]);
        assert_that!(evidence, len(eq(2)));
        assert_location_contains(evidence, "a.rs");
        assert_location_contains(evidence, "b.rs");
    }

    #[test]
    fn duplicated_code_returns_one_evaluation_per_clone_group() {
        let dir = clone_pair_dir();

        let evals = check_dir(dir.path());

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
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "a.rs", "let x = 1 + 2;");
        write_file(dir.path(), "b.rs", "if true { return false; }");

        let evals = check_dir(dir.path());

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn detects_typescript_duplications() {
        let dir = TempDir::new().unwrap();
        write_file(
            dir.path(),
            "a.ts",
            "function foo(x: number): number { return x + 1; }",
        );
        write_file(
            dir.path(),
            "b.ts",
            "function bar(y: number): number { return y + 1; }",
        );

        let evals = check_dir(dir.path());

        assert_that!(evals, len(eq(1)));
    }

    #[test]
    fn focus_file_only_reports_clone_groups_involving_that_file() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "a.rs", "fn foo(x: i32) -> i32 { x + 1 }");
        write_file(dir.path(), "b.rs", "fn bar(y: i32) -> i32 { y + 1 }");
        write_file(
            dir.path(),
            "c.rs",
            "const A: [i32; 5] = [10, 20, 30, 40, 50];",
        );
        write_file(
            dir.path(),
            "d.rs",
            "const B: [u32; 5] = [60, 70, 80, 90, 100];",
        );

        let evals = check_focused(dir.path(), &[dir.path().join("a.rs")]);

        assert_that!(evals, len(eq(1)));
        let evidence = unwrap_evidence(&evals[0]);
        assert_location_contains(evidence, "a.rs");
        assert_location_contains(evidence, "b.rs");
    }

    #[test]
    fn unsupported_focus_file_produces_errored_evaluation() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "script.py", "def foo(): pass");

        let evals = check_focused(dir.path(), &[dir.path().join("script.py")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_error());
        assert_that!(evals[0].target, contains_substring("script.py"));
    }

    #[test]
    fn unreadable_focus_file_produces_errored_evaluation() {
        let dir = TempDir::new().unwrap();

        let evals = check_focused(dir.path(), &[dir.path().join("missing.rs")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_error());
        assert_that!(evals[0].target, contains_substring("missing.rs"));
    }

    #[test]
    fn focus_file_without_clones_passes() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "clean.rs", "fn unique_stuff() -> bool { true }");
        write_file(dir.path(), "a.rs", "fn foo(x: i32) -> i32 { x + 1 }");
        write_file(dir.path(), "b.rs", "fn bar(y: i32) -> i32 { y + 1 }");

        let evals = check_focused(dir.path(), &[dir.path().join("clean.rs")]);

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }

    #[test]
    fn multiple_focus_files_report_clones_involving_any_of_them() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "a.rs", "fn foo(x: i32) -> i32 { x + 1 }");
        write_file(dir.path(), "b.rs", "fn bar(y: i32) -> i32 { y + 1 }");
        write_file(
            dir.path(),
            "c.rs",
            "const A: [i32; 5] = [10, 20, 30, 40, 50];",
        );
        write_file(
            dir.path(),
            "d.rs",
            "const B: [u32; 5] = [60, 70, 80, 90, 100];",
        );

        let evals = check_focused(
            dir.path(),
            &[dir.path().join("a.rs"), dir.path().join("c.rs")],
        );

        assert_that!(evals, len(eq(2)));
    }

    #[test]
    fn single_file_without_duplication_passes() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "a.rs", "fn foo(x: i32) -> i32 { x + 1 }");

        let evals = check_dir(dir.path());

        assert_that!(evals, len(eq(1)));
        assert!(evals[0].is_pass());
    }
}
