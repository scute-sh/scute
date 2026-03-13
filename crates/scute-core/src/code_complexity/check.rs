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

    let focus: Vec<PathBuf> = focus_files
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();

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

fn construct_label(construct: score::Construct) -> &'static str {
    match construct {
        score::Construct::If => "if",
        score::Construct::For => "for",
        score::Construct::While => "while",
        score::Construct::Loop => "loop",
        score::Construct::Match => "match",
    }
}

fn flow_break_label(construct: score::Construct) -> &'static str {
    match construct {
        score::Construct::For | score::Construct::While | score::Construct::Loop => "loop",
        score::Construct::If => "conditional",
        score::Construct::Match => "expression",
    }
}

fn jump_keyword_label(keyword: score::JumpKeyword) -> &'static str {
    match keyword {
        score::JumpKeyword::Break => "break",
        score::JumpKeyword::Continue => "continue",
    }
}

fn format_nesting_chain(chain: &[score::Construct]) -> String {
    chain
        .iter()
        .map(|c| construct_label(*c))
        .collect::<Vec<_>>()
        .join(" > ")
}

fn pluralize_levels(n: u64) -> &'static str {
    if n == 1 { "level" } else { "levels" }
}

fn format_ops(operators: &[String]) -> String {
    let mut seen: Vec<&str> = vec![];
    for op in operators {
        if !seen.contains(&op.as_str()) {
            seen.push(op);
        }
    }
    let quoted: Vec<String> = seen.iter().map(|o| format!("'{o}'")).collect();
    let prefix = if seen.len() > 1 { "mixed " } else { "" };
    format!("{prefix}{}", quoted.join(" and "))
}

fn format_evidence(c: &score::Contributor, path: &Path) -> Evidence {
    let location = Some(format!("{}:{}", path.display(), c.line));
    let text = |s: &str| Some(Expected::Text(s.into()));

    let (rule, found, expected) = match &c.kind {
        score::ContributorKind::Structural {
            construct,
            nesting_depth,
            nesting_chain,
        } if *nesting_depth > 0 => {
            let name = construct_label(*construct);
            let chain = format_nesting_chain(nesting_chain);
            let levels = pluralize_levels(*nesting_depth);
            (
                "nesting",
                format!(
                    "'{name}' nested {nesting_depth} {levels}: '{chain}' (+{})",
                    c.increment
                ),
                text("extract inner block into a function"),
            )
        }
        score::ContributorKind::Structural { construct, .. } => (
            "flow break",
            format!(
                "'{}' {} (+{})",
                construct_label(*construct),
                flow_break_label(*construct),
                c.increment
            ),
            None,
        ),
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
            format!(
                "'{}' to label '{label}' (+{})",
                jump_keyword_label(*keyword),
                c.increment
            ),
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

    #[test]
    fn evidence_shows_contributor_breakdown() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn hello() { if true {} }").unwrap();

        let evals = check_dir(dir.path());
        let crate::Outcome::Completed { evidence, .. } = &evals[0].outcome else {
            panic!("expected completed");
        };

        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].rule.as_deref(), Some("flow break"));
        assert!(evidence[0].location.as_ref().unwrap().contains("a.rs:1"));
        assert!(evidence[0].found.contains("'if' conditional (+1)"));
        assert!(evidence[0].expected.is_none());
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
