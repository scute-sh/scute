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

/// Validate and canonicalize focus files.
///
/// Checks that each file has a supported extension and exists on disk.
/// Returns canonical paths on success, or errored evaluations on failure.
///
/// # Errors
///
/// Returns errored [`Evaluation`](crate::Evaluation)s when any file has an
/// unsupported extension or cannot be read.
pub fn validate_focus_files(
    files: &[PathBuf],
    supported_extensions: &[&str],
    supported_msg: &str,
) -> Result<Vec<PathBuf>, Vec<crate::Evaluation>> {
    let mut canonical = Vec::new();
    let mut errors = Vec::new();
    for path in files {
        match validate_focus_file(path, supported_extensions, supported_msg) {
            Ok(p) => canonical.push(p),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(canonical)
    } else {
        Err(errors)
    }
}

fn validate_focus_file(
    path: &Path,
    supported_extensions: &[&str],
    supported_msg: &str,
) -> Result<PathBuf, crate::Evaluation> {
    let has_supported_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| supported_extensions.contains(&ext));
    if !has_supported_ext {
        return Err(crate::Evaluation::errored(
            path.display().to_string(),
            crate::ExecutionError {
                code: "unsupported_language".into(),
                message: format!("unsupported file type: {}", path.display()),
                recovery: supported_msg.into(),
            },
        ));
    }
    path.canonicalize().map_err(|_| {
        crate::Evaluation::errored(
            path.display().to_string(),
            crate::ExecutionError {
                code: "unreadable_file".into(),
                message: format!("cannot read file: {}", path.display()),
                recovery: "check that the file exists and is readable".into(),
            },
        )
    })
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
    fn focus_files_rejects_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let py_file = dir.path().join("script.py");
        fs::write(&py_file, "").unwrap();

        let result = validate_focus_files(&[py_file], &["rs"], "only Rust files are supported");

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].is_error());
        assert!(errors[0].target.contains("script.py"));
    }

    #[test]
    fn focus_files_rejects_nonexistent_file() {
        let missing = PathBuf::from("/does/not/exist.rs");

        let result = validate_focus_files(&[missing], &["rs"], "only Rust files are supported");

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].is_error());
    }

    #[test]
    fn focus_files_canonicalizes_valid_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("real.rs");
        fs::write(&file, "").unwrap();

        let result = validate_focus_files(
            std::slice::from_ref(&file),
            &["rs"],
            "only Rust files are supported",
        );

        let paths = result.unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], file.canonicalize().unwrap());
    }

    #[test]
    fn focus_files_returns_empty_for_no_files() {
        let result: Result<Vec<PathBuf>, _> =
            validate_focus_files(&[], &["rs"], "only Rust files are supported");

        assert!(result.unwrap().is_empty());
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
