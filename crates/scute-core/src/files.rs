use std::path::{Path, PathBuf};
use std::{fmt, io};

use ignore::WalkBuilder;
use ignore::overrides::{Override, OverrideBuilder};

/// Source code with its file path, ready for check processing.
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

/// Glob patterns for files to leave out, matched relative to a root directory.
pub struct Exclusions(Override);

impl Exclusions {
    /// Patterns that aren't valid globs are ignored.
    #[must_use]
    pub fn new(root: &Path, patterns: &[String]) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let mut builder = OverrideBuilder::new(root);
        for pattern in patterns {
            builder.add(&format!("!{pattern}")).ok();
        }
        Self(builder.build().unwrap_or_else(|_| Override::empty()))
    }

    #[must_use]
    pub fn none() -> Self {
        Self(Override::empty())
    }

    fn matches(&self, file: &Path) -> bool {
        self.0.matched(file, false).is_ignore()
    }
}

pub fn walk_source_files(
    dir: &Path,
    skip_ignored: bool,
    exclusions: &Exclusions,
) -> impl Iterator<Item = ignore::DirEntry> {
    let mut builder = WalkBuilder::new(dir);
    builder
        .standard_filters(skip_ignored)
        .overrides(exclusions.0.clone());

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
    Unreadable(io::Error),
    ExpectedDirectory,
    InvalidDirectory(io::Error),
}

/// Source files found at a set of paths, and the paths that couldn't be resolved.
#[derive(Debug, Default)]
pub struct ResolvedPaths {
    pub files: Vec<PathBuf>,
    pub invalid: Vec<InvalidPath>,
}

/// Resolve mixed file/directory paths into a flat list of source files.
///
/// Directories are walked to discover files with a supported extension.
/// Files given directly must exist, and are kept only if their extension is
/// supported. Files matching `exclusions` are left out either way. A path
/// that doesn't exist or can't be read is reported in
/// [`ResolvedPaths::invalid`] without stopping the others.
#[must_use]
pub fn resolve_paths(
    paths: &[PathBuf],
    supported_extensions: &[&str],
    exclusions: &Exclusions,
) -> ResolvedPaths {
    let mut resolved = ResolvedPaths::default();
    for path in paths {
        match resolve_path(path, supported_extensions, exclusions) {
            Ok(files) => resolved.files.extend(files),
            Err(invalid) => resolved.invalid.push(invalid),
        }
    }
    resolved
}

fn resolve_path(
    path: &Path,
    supported_extensions: &[&str],
    exclusions: &Exclusions,
) -> Result<Vec<PathBuf>, InvalidPath> {
    if path.is_dir() {
        let dir = validate_source_dir(path)?;
        return Ok(discover_files(&dir, supported_extensions, exclusions));
    }
    let file = resolve_file(path)?;
    Ok(
        if has_extension(path, supported_extensions) && !exclusions.matches(&file) {
            vec![file]
        } else {
            vec![]
        },
    )
}

fn discover_files(dir: &Path, extensions: &[&str], exclusions: &Exclusions) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = walk_source_files(dir, true, exclusions)
        .filter(|e| has_extension(e.path(), extensions))
        .map(ignore::DirEntry::into_path)
        .collect();
    files.sort();
    files
}

fn resolve_file(path: &Path) -> Result<PathBuf, InvalidPath> {
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
        walk_source_files(dir, false, &Exclusions::new(dir, exclude))
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

            let result = resolve_paths(&[dir.path("main.rs")], &["rs"], &Exclusions::none());

            assert_that!(result.files, len(eq(1)));
        }

        #[test]
        fn resolves_directory() {
            let dir = TestDir::new().file("a.rs").file("b.rs");

            let result = resolve_paths(&[dir.root()], &["rs"], &Exclusions::none());

            assert_that!(result.files, len(eq(2)));
        }

        #[test]
        fn resolves_mixed_files_and_directories() {
            let dir = TestDir::new().file("main.rs").file("src/lib.rs");

            let result = resolve_paths(
                &[dir.path("main.rs"), dir.path("src")],
                &["rs"],
                &Exclusions::none(),
            );

            assert_that!(result.files, len(eq(2)));
        }

        #[test]
        fn returns_empty_for_empty_input() {
            let result = resolve_paths(&[], &["rs"], &Exclusions::none());

            assert_that!(result.files, is_empty());
        }

        #[test]
        fn reports_invalid_path_and_still_resolves_the_valid_ones() {
            let dir = TestDir::new().file("good.rs");
            let bad = PathBuf::from("/nonexistent/file.rs");

            let resolved = resolve_paths(
                &[bad.clone(), dir.path("good.rs")],
                &["rs"],
                &Exclusions::none(),
            );

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

            let result = resolve_paths(
                &[dir.root()],
                &["rs"],
                &Exclusions::new(&dir.root(), &["gen/**".into()]),
            );

            let files = result.files;
            assert_eq!(files.len(), 1);
            assert!(files[0].ends_with("keep.rs"));
        }

        #[cfg(unix)]
        #[test]
        fn excludes_matching_files_when_root_is_reached_through_a_symlink() {
            let dir = TestDir::new().file("keep.rs").file("gen/skip.rs");
            let links = tempfile::tempdir().unwrap();
            let root = links.path().join("project");
            std::os::unix::fs::symlink(dir.root(), &root).unwrap();

            let result = resolve_paths(
                &[root.clone(), root.join("gen/skip.rs")],
                &["rs"],
                &Exclusions::new(&root, &["gen/**".into()]),
            );

            assert_that!(
                result.files,
                elements_are![eq(&dir.path("keep.rs").canonicalize().unwrap())]
            );
        }

        #[test]
        fn skips_file_with_unsupported_extension() {
            let dir = TestDir::new().file("script.py");

            let result = resolve_paths(&[dir.path("script.py")], &["rs"], &Exclusions::none());

            assert_that!(result.files, is_empty());
            assert_that!(result.invalid, is_empty());
        }

        #[test]
        fn preserves_os_error_for_unreadable_file() {
            let result = resolve_paths(
                &[PathBuf::from("/nonexistent/file.rs")],
                &["rs"],
                &Exclusions::none(),
            );

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
