use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::score;
use crate::files;
use crate::{Evaluation, Evidence, ExecutionError, Thresholds};

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
            warn: Some(15),
            fail: Some(25),
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

    let focus: Vec<PathBuf> = focus_files
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();

    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut evaluations = Vec::new();

    for path in &rust_files {
        if !focus.is_empty() && !focus.contains(path) {
            continue;
        }

        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };

        for func in score::score_functions(&source, &language) {
            let target = format!("{}:{}:{}", path.display(), func.line, func.name);
            evaluations.push(Evaluation::completed(
                target,
                func.score,
                thresholds.clone(),
                vec![Evidence {
                    rule: None,
                    location: Some(format!("{}:{}", path.display(), func.line)),
                    found: source_line(&source, func.line),
                    expected: None,
                }],
            ));
        }
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

fn discover_rust_files(dir: &Path, exclude: &[String]) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = files::walk_source_files(dir, true, exclude)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(ignore::DirEntry::into_path)
        .collect();
    result.sort();
    result
}

fn source_line(source: &str, line: usize) -> String {
    source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        assert!(evals[0].is_pass()); // score 1, default warn 15
    }

    #[test]
    fn evidence_includes_location_and_source_line() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn hello() { if true {} }").unwrap();

        let evals = check_dir(dir.path());
        let crate::Outcome::Completed { evidence, .. } = &evals[0].outcome else {
            panic!("expected completed");
        };

        assert_eq!(evidence.len(), 1);
        assert!(evidence[0].location.as_ref().unwrap().contains("a.rs:1"));
        assert_eq!(evidence[0].found, "fn hello() { if true {} }");
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
}
