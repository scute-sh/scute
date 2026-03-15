use std::path::{Path, PathBuf};
use std::{fmt, io};

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

/// Validate and canonicalize a directory path.
///
/// # Errors
///
/// Returns `InvalidPath` if the path doesn't exist, isn't a directory,
/// or cannot be canonicalized.
pub fn validate_source_dir(source_dir: &Path) -> Result<PathBuf, InvalidPath> {
    let canonical = source_dir.canonicalize().map_err(|e| InvalidPath {
        path: source_dir.display().to_string(),
        kind: InvalidPathKind::InvalidDirectory(e),
    })?;
    if !canonical.is_dir() {
        return Err(InvalidPath {
            path: source_dir.display().to_string(),
            kind: InvalidPathKind::ExpectedDirectory,
        });
    }
    Ok(canonical)
}

/// A path that couldn't be resolved.
#[derive(Debug)]
pub struct InvalidPath {
    pub path: String,
    pub kind: InvalidPathKind,
}

impl fmt::Display for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            InvalidPathKind::UnsupportedExtension => {
                write!(f, "unsupported file type: {}", self.path)
            }
            InvalidPathKind::Unreadable(e) => write!(f, "cannot read {}: {e}", self.path),
            InvalidPathKind::ExpectedDirectory => {
                write!(f, "not a directory: {}", self.path)
            }
            InvalidPathKind::InvalidDirectory(e) => {
                write!(f, "cannot read directory {}: {e}", self.path)
            }
        }
    }
}

impl std::error::Error for InvalidPath {}

#[derive(Debug)]
pub enum InvalidPathKind {
    UnsupportedExtension,
    Unreadable(io::Error),
    ExpectedDirectory,
    InvalidDirectory(io::Error),
}

/// Returns `paths` as-is if non-empty, otherwise a single-element vec
/// containing `default`.
#[must_use]
pub fn paths_or_default(paths: Vec<PathBuf>, default: &Path) -> Vec<PathBuf> {
    if paths.is_empty() {
        vec![default.to_path_buf()]
    } else {
        paths
    }
}

/// Resolve mixed file/directory paths into a flat list of source files.
///
/// Each path is classified: files are validated individually (extension +
/// readability), directories are walked to discover matching files.
///
/// Fails fast on the first invalid path.
///
/// # Errors
///
/// Returns `InvalidPath` if any path has an unsupported extension,
/// doesn't exist, or is an unreadable directory.
pub fn resolve_paths(
    paths: &[PathBuf],
    supported_extensions: &[&str],
    exclude: &[String],
) -> Result<Vec<PathBuf>, InvalidPath> {
    let mut resolved = Vec::new();
    for path in paths {
        if path.is_dir() {
            let dir = validate_source_dir(path)?;
            resolved.extend(discover_files(&dir, supported_extensions, exclude));
        } else {
            resolved.push(resolve_file(path, supported_extensions)?);
        }
    }
    Ok(resolved)
}

fn discover_files(dir: &Path, extensions: &[&str], exclude: &[String]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = walk_source_files(dir, true, exclude)
        .filter(|e| has_extension(e.path(), extensions))
        .map(ignore::DirEntry::into_path)
        .collect();
    files.sort();
    files
}

fn resolve_file(path: &Path, supported_extensions: &[&str]) -> Result<PathBuf, InvalidPath> {
    if !has_extension(path, supported_extensions) {
        return Err(InvalidPath {
            path: path.display().to_string(),
            kind: InvalidPathKind::UnsupportedExtension,
        });
    }
    path.canonicalize().map_err(|e| InvalidPath {
        path: path.display().to_string(),
        kind: InvalidPathKind::Unreadable(e),
    })
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| extensions.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;

    #[test]
    fn validates_existing_directory() {
        let dir = tempfile::tempdir().unwrap();

        let result = validate_source_dir(dir.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir.path().canonicalize().unwrap());
    }

    #[test]
    fn rejects_nonexistent_directory() {
        let result = validate_source_dir(Path::new("/does/not/exist"));

        assert_that!(
            result,
            err(field!(
                InvalidPath.kind,
                pat!(InvalidPathKind::InvalidDirectory(_))
            ))
        );
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
        let result = validate_focus_files(&[], &["rs"], "only Rust files are supported");

        assert_that!(result, ok(is_empty()));
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

    #[test]
    fn rejects_file_passed_as_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not_a_dir.rs");
        fs::write(&file, "").unwrap();

        let result = validate_source_dir(&file);

        assert_that!(
            result,
            err(field!(
                InvalidPath.kind,
                pat!(InvalidPathKind::ExpectedDirectory)
            ))
        );
    }

    mod resolve_paths_tests {
        use super::*;
        use scute_test_utils::TestDir;

        #[test]
        fn resolves_single_file() {
            let dir = TestDir::new().file("main.rs");

            let result = resolve_paths(&[dir.path("main.rs")], &["rs"], &[]);

            assert_that!(result, ok(len(eq(1))));
        }

        #[test]
        fn resolves_directory() {
            let dir = TestDir::new().file("a.rs").file("b.rs");

            let result = resolve_paths(&[dir.root()], &["rs"], &[]);

            assert_that!(result, ok(len(eq(2))));
        }

        #[test]
        fn resolves_mixed_files_and_directories() {
            let dir = TestDir::new().file("main.rs").file("src/lib.rs");

            let result = resolve_paths(&[dir.path("main.rs"), dir.path("src")], &["rs"], &[]);

            assert_that!(result, ok(len(eq(2))));
        }

        #[test]
        fn returns_empty_for_empty_input() {
            let result = resolve_paths(&[], &["rs"], &[]);

            assert_that!(result, ok(is_empty()));
        }

        #[test]
        fn fails_fast_on_first_invalid_path() {
            let dir = TestDir::new().file("good.rs");
            let bad = PathBuf::from("/nonexistent/file.rs");

            let result = resolve_paths(&[bad.clone(), dir.path("good.rs")], &["rs"], &[]);

            let err = result.unwrap_err();
            assert_eq!(err.path, bad.display().to_string());
        }

        #[test]
        fn forwards_exclude_patterns_to_directory_walk() {
            let dir = TestDir::new().file("keep.rs").file("gen/skip.rs");

            let result = resolve_paths(&[dir.root()], &["rs"], &["gen/**".into()]);

            let files = result.unwrap();
            assert_eq!(files.len(), 1);
            assert!(files[0].ends_with("keep.rs"));
        }

        #[test]
        fn rejects_unsupported_extension() {
            let dir = TestDir::new().file("script.py");

            let result = resolve_paths(&[dir.path("script.py")], &["rs"], &[]);

            assert_that!(
                result,
                err(field!(
                    InvalidPath.kind,
                    pat!(InvalidPathKind::UnsupportedExtension)
                ))
            );
        }

        #[test]
        fn preserves_os_error_for_unreadable_file() {
            let result = resolve_paths(&[PathBuf::from("/nonexistent/file.rs")], &["rs"], &[]);

            assert_that!(
                result,
                err(field!(
                    InvalidPath.kind,
                    pat!(InvalidPathKind::Unreadable(_))
                ))
            );
        }
    }
}
