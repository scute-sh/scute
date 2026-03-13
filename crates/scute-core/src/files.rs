use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

pub fn walk_source_files(
    dir: &Path,
    skip_ignored: bool,
    exclude: &[String],
) -> impl Iterator<Item = ignore::DirEntry> {
    let mut builder = WalkBuilder::new(dir);
    builder.standard_filters(skip_ignored);

    if !exclude.is_empty() {
        let mut overrides = ignore::overrides::OverrideBuilder::new(dir);
        for pattern in exclude {
            overrides.add(&format!("!{pattern}")).ok();
        }
        if let Ok(built) = overrides.build() {
            builder.overrides(built);
        }
    }

    builder
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
}

/// # Errors
///
/// Returns `ExecutionError` if the path cannot be canonicalized.
pub fn validate_source_dir(source_dir: &Path) -> Result<PathBuf, crate::ExecutionError> {
    source_dir
        .canonicalize()
        .map_err(|e| crate::ExecutionError {
            code: "invalid_target".into(),
            message: format!("cannot read directory {}: {e}", source_dir.display()),
            recovery: "check that the path exists and is a directory".into(),
        })
}
