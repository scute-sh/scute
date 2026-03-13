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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_existing_directory() {
        let dir = tempfile::tempdir().unwrap();

        let result = validate_source_dir(dir.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir.path().canonicalize().unwrap());
    }

    #[test]
    fn rejects_nonexistent_path() {
        let result = validate_source_dir(Path::new("/does/not/exist"));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "invalid_target");
    }

    fn walk(dir: &Path, exclude: &[String]) -> Vec<PathBuf> {
        walk_source_files(dir, false, exclude)
            .map(ignore::DirEntry::into_path)
            .collect()
    }

    #[test]
    fn walks_only_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.rs"), "").unwrap();

        assert_eq!(walk(dir.path(), &[]).len(), 2);
    }

    #[test]
    fn excludes_matching_patterns() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("keep.rs"), "").unwrap();
        fs::create_dir(dir.path().join("vendor")).unwrap();
        fs::write(dir.path().join("vendor/skip.rs"), "").unwrap();

        let files = walk(dir.path(), &["vendor/**".into()]);

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("keep.rs"));
    }
}
