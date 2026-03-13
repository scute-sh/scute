use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::score;
use crate::{Evaluation, Evidence, ExecutionError, Thresholds};

pub const CHECK_NAME: &str = "cognitive-complexity";

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
    let canonical_dir = source_dir.canonicalize().map_err(|e| ExecutionError {
        code: "invalid_target".into(),
        message: format!("cannot read directory {}: {e}", source_dir.display()),
        recovery: "check that the path exists and is a directory".into(),
    })?;

    let thresholds = definition.thresholds();
    let exclude = definition.exclude.as_deref().unwrap_or_default();
    let files = discover_rust_files(&canonical_dir, exclude);

    let focus: Vec<PathBuf> = focus_files
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();

    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut evaluations = Vec::new();

    for path in &files {
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
        return Ok(vec![Evaluation::completed(
            source_dir.display().to_string(),
            0,
            thresholds,
            vec![],
        )]);
    }

    // Only return functions that exceed thresholds
    let flagged: Vec<_> = evaluations.into_iter().filter(|e| !e.is_pass()).collect();

    if flagged.is_empty() {
        Ok(vec![Evaluation::completed(
            source_dir.display().to_string(),
            0,
            thresholds,
            vec![],
        )])
    } else {
        Ok(flagged)
    }
}

fn discover_rust_files(dir: &Path, exclude: &[String]) -> Vec<PathBuf> {
    let mut builder = ignore::WalkBuilder::new(dir);
    builder.standard_filters(true);

    if !exclude.is_empty() {
        let mut overrides = ignore::overrides::OverrideBuilder::new(dir);
        for pattern in exclude {
            overrides.add(&format!("!{pattern}")).ok();
        }
        if let Ok(built) = overrides.build() {
            builder.overrides(built);
        }
    }

    let mut files: Vec<PathBuf> = builder
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(ignore::DirEntry::into_path)
        .collect();
    files.sort();
    files
}

fn source_line(source: &str, line: usize) -> String {
    source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .trim()
        .to_string()
}
