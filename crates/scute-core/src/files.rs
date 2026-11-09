use std::path::{Path, PathBuf};
use std::{fmt, io};

use ignore::WalkBuilder;

/// Source code with its file path, ready for check processing.
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

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

/// Source files found at a set of paths, and the paths that didn't lead to any.
#[derive(Debug, Default)]
pub struct ResolvedPaths {
    pub files: Vec<PathBuf>,
    pub invalid: Vec<InvalidPath>,
}

/// Resolve mixed file/directory paths into a flat list of source files.
///
/// Files are validated individually (extension + readability), directories
/// are walked to discover matching files. A path that can't be resolved is
/// reported in [`ResolvedPaths::invalid`] without stopping the others.
#[must_use]
pub fn resolve_paths(
    paths: &[PathBuf],
    supported_extensions: &[&str],
    exclude: &[String],
) -> ResolvedPaths {
    let mut resolved = ResolvedPaths::default();
    for path in paths {
        match resolve_path(path, supported_extensions, exclude) {
            Ok(files) => resolved.files.extend(files),
            Err(invalid) => resolved.invalid.push(invalid),
        }
    }
    resolved
}

fn resolve_path(
    path: &Path,
    supported_extensions: &[&str],
    exclude: &[String],
) -> Result<Vec<PathBuf>, InvalidPath> {
    if path.is_dir() {
        let dir = validate_source_dir(path)?;
        return Ok(discover_files(&dir, supported_extensions, exclude));
    }
    resolve_file(path, supported_extensions).map(|file| vec![file])
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

            assert_that!(result.files, len(eq(1)));
        }

        #[test]
        fn resolves_directory() {
            let dir = TestDir::new().file("a.rs").file("b.rs");

            let result = resolve_paths(&[dir.root()], &["rs"], &[]);

            assert_that!(result.files, len(eq(2)));
        }

        #[test]
        fn resolves_mixed_files_and_directories() {
            let dir = TestDir::new().file("main.rs").file("src/lib.rs");

            let result = resolve_paths(&[dir.path("main.rs"), dir.path("src")], &["rs"], &[]);

            assert_that!(result.files, len(eq(2)));
        }

        #[test]
        fn returns_empty_for_empty_input() {
            let result = resolve_paths(&[], &["rs"], &[]);

            assert_that!(result.files, is_empty());
        }

        #[test]
        fn reports_invalid_path_and_still_resolves_the_valid_ones() {
            let dir = TestDir::new().file("good.rs");
            let bad = PathBuf::from("/nonexistent/file.rs");

            let resolved = resolve_paths(&[bad.clone(), dir.path("good.rs")], &["rs"], &[]);

            assert_that!(
                resolved.files,
                elements_are![eq(&dir.path("good.rs").canonicalize().unwrap())]
            );
            assert_that!(
                resolved.invalid,
                elements_are![field!(InvalidPath.path, eq(&bad.display().to_string()))]
            );
        }

        #[test]
        fn forwards_exclude_patterns_to_directory_walk() {
            let dir = TestDir::new().file("keep.rs").file("gen/skip.rs");

            let result = resolve_paths(&[dir.root()], &["rs"], &["gen/**".into()]);

            let files = result.files;
            assert_eq!(files.len(), 1);
            assert!(files[0].ends_with("keep.rs"));
        }

        #[test]
        fn rejects_unsupported_extension() {
            let dir = TestDir::new().file("script.py");

            let result = resolve_paths(&[dir.path("script.py")], &["rs"], &[]);

            assert_that!(
                result.invalid,
                elements_are![field!(
                    InvalidPath.kind,
                    pat!(InvalidPathKind::UnsupportedExtension)
                )]
            );
        }

        #[test]
        fn preserves_os_error_for_unreadable_file() {
            let result = resolve_paths(&[PathBuf::from("/nonexistent/file.rs")], &["rs"], &[]);

            assert_that!(
                result.invalid,
                elements_are![field!(
                    InvalidPath.kind,
                    pat!(InvalidPathKind::Unreadable(_))
                )]
            );
        }
    }
}
