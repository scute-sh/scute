#![allow(
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::return_self_not_must_use
)]

mod cli;
pub mod mcp;
mod project;

use std::path::{Path, PathBuf};

use cli::CliBackend;
use mcp::McpBackend;
pub use project::{PackageManager, TestProject};
use tempfile::TempDir;

/// Lightweight temp directory builder for unit tests that just need files on disk.
///
/// ```
/// use scute_test_utils::TestDir;
///
/// let t = TestDir::new()
///     .file("src/main.rs")                       // empty file
///     .source_file("lib.rs", "fn hello() {}");   // file with content
///
/// let root = t.root();        // path to the temp directory
/// let file = t.path("lib.rs"); // path to a specific file
/// ```
pub struct TestDir {
    dir: TempDir,
}

#[allow(clippy::new_without_default)]
impl TestDir {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn file(self, name: &str) -> Self {
        self.source_file(name, "")
    }

    pub fn source_file(self, name: &str, content: &str) -> Self {
        let path = self.dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
        self
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }
}

/// How the check process terminated, from the interface's perspective.
///
/// CLI maps exit codes: 0 → Success, 1 → Failure, 2 → Error.
/// MCP maps `isError` + JSON shape: no error → Success, isError + error object or
/// errored findings → Error, isError otherwise → Failure. Errors take precedence
/// over failures, like the CLI's exit codes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ExitStatus {
    Success,
    Failure,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub enum Interface {
    /// The CLI with input as arguments and stdin closed.
    Cli,
    /// The CLI with input piped into stdin, paths one per line. No input
    /// pipes an empty stdin.
    CliStdin,
    /// The MCP server, with input as tool parameters.
    Mcp,
}

impl std::fmt::Display for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cli => write!(f, "cli"),
            Self::CliStdin => write!(f, "cli_stdin"),
            Self::Mcp => write!(f, "mcp"),
        }
    }
}

/// Typed check parameters. Each variant maps to a check's inputs.
///
/// To add a new check:
/// 1. Add a variant here with the check's parameters
/// 2. Add a constructor on [`Scute`] that takes mandatory params and returns a builder
/// 3. Put optional params as builder methods, `.run()` executes
/// 4. Implement the variant in both backends' `run_check`
pub(crate) enum CheckInput {
    CommitMessage {
        message: String,
    },
    DependencyFreshness {
        path: Option<String>,
    },
    CodeComplexity {
        paths: Option<Vec<String>>,
    },
    CodeSimilarity {
        source_dir: Option<String>,
        files: Option<Vec<String>>,
    },
}

trait Backend {
    fn run_check(&self, dir: TempDir, working_dir: &Path, input: &CheckInput) -> CheckResult;
    fn list_checks(&self, dir: TempDir) -> ListChecksResult;
    fn describe_check(&self, dir: TempDir, name: &str) -> CheckDescription;
}

pub struct Scute {
    backend: Box<dyn Backend>,
    project: TestProject,
    cwd: Option<String>,
}

impl Scute {
    pub fn new(interface: Interface) -> Self {
        match interface {
            Interface::Cli => Self::cli(),
            Interface::CliStdin => Self::cli_stdin(),
            Interface::Mcp => Self::mcp(),
        }
    }

    pub fn cli() -> Self {
        Self {
            backend: Box::new(CliBackend { via_stdin: false }),
            project: TestProject::empty(),
            cwd: None,
        }
    }

    pub fn cli_stdin() -> Self {
        Self {
            backend: Box::new(CliBackend { via_stdin: true }),
            project: TestProject::empty(),
            cwd: None,
        }
    }

    pub fn mcp() -> Self {
        Self {
            backend: Box::new(McpBackend),
            project: TestProject::empty(),
            cwd: None,
        }
    }

    /// Set up the project's manifest with this package manager, so it can
    /// hold dependencies. Without one, the project is a plain directory with
    /// only the files you add, which is all code checks need: the language
    /// comes from the files themselves.
    ///
    /// ```no_run
    /// use scute_test_utils::{Interface, PackageManager, Scute};
    ///
    /// Scute::new(Interface::Cli)
    ///     .package_manager(PackageManager::Cargo)
    ///     .dependency("itoa", "=0.4.8")
    ///     .dependency_freshness()
    ///     .run()
    ///     .expect_fail();
    /// ```
    pub fn package_manager(mut self, package_manager: PackageManager) -> Self {
        self.project = self.project.package_manager(package_manager);
        self
    }

    pub fn dependency(mut self, name: &str, version: &str) -> Self {
        self.project = self.project.dependency(name, version);
        self
    }

    pub fn dev_dependency(mut self, name: &str, version: &str) -> Self {
        self.project = self.project.dev_dependency(name, version);
        self
    }

    pub fn source_file(mut self, name: &str, content: &str) -> Self {
        self.project = self.project.source_file(name, content);
        self
    }

    pub fn scute_config(mut self, yaml: &str) -> Self {
        self.project = self.project.scute_config(yaml);
        self
    }

    /// Run the check from a subdirectory instead of the project root.
    pub fn cwd(mut self, subdir: &str) -> Self {
        self.cwd = Some(subdir.into());
        self
    }

    pub fn commit_message(self, message: &str) -> CommitMessageCheck {
        CommitMessageCheck {
            scute: self,
            message: message.into(),
        }
    }

    pub fn dependency_freshness(self) -> DependencyFreshnessCheck {
        DependencyFreshnessCheck {
            scute: self,
            path: None,
        }
    }

    pub fn code_complexity(self) -> CodeComplexityCheck {
        CodeComplexityCheck {
            scute: self,
            paths: None,
        }
    }

    pub fn code_similarity(self) -> CodeSimilarityCheck {
        CodeSimilarityCheck {
            scute: self,
            source_dir: None,
            files: None,
        }
    }

    pub fn list_checks(self) -> ListChecksResult {
        let dir = self.project.build();
        self.backend.list_checks(dir)
    }

    /// What a caller learns about a check before running it: its `--help`
    /// on the CLI, its tool description over MCP.
    pub fn describe_check(self, name: &str) -> CheckDescription {
        let dir = self.project.build();
        self.backend.describe_check(dir, name)
    }

    fn run_check(self, input: &CheckInput) -> CheckResult {
        let dir = self.project.build();
        let working_dir = resolve_working_dir(&dir, self.cwd.as_deref());
        self.backend.run_check(dir, &working_dir, input)
    }
}

fn resolve_working_dir(dir: &TempDir, cwd: Option<&str>) -> PathBuf {
    match cwd {
        Some(subdir) => {
            let path = dir.path().join(subdir);
            std::fs::create_dir_all(&path).expect("failed to create cwd subdir");
            path
        }
        None => dir.path().to_path_buf(),
    }
}

pub struct CommitMessageCheck {
    scute: Scute,
    message: String,
}

impl CommitMessageCheck {
    pub fn run(self) -> CheckResult {
        self.scute.run_check(&CheckInput::CommitMessage {
            message: self.message,
        })
    }
}

pub struct DependencyFreshnessCheck {
    scute: Scute,
    path: Option<String>,
}

impl DependencyFreshnessCheck {
    pub fn path(mut self, path: &str) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn run(self) -> CheckResult {
        self.scute
            .run_check(&CheckInput::DependencyFreshness { path: self.path })
    }
}

pub struct CodeComplexityCheck {
    scute: Scute,
    paths: Option<Vec<String>>,
}

impl CodeComplexityCheck {
    /// Hands the check these paths. Without this call, the check gets no
    /// input at all. An empty slice is an explicitly empty list, which only
    /// `Mcp` can express (`[]`): both CLI interfaces refuse it.
    pub fn paths(mut self, paths: &[&str]) -> Self {
        self.paths = Some(paths.iter().map(|s| (*s).into()).collect());
        self
    }

    pub fn run(self) -> CheckResult {
        self.scute
            .run_check(&CheckInput::CodeComplexity { paths: self.paths })
    }
}

pub struct CodeSimilarityCheck {
    scute: Scute,
    source_dir: Option<String>,
    files: Option<Vec<String>>,
}

impl CodeSimilarityCheck {
    pub fn source_dir(mut self, dir: &str) -> Self {
        self.source_dir = Some(dir.into());
        self
    }

    /// Focuses the check on these files. Without this call, the check gets
    /// no input at all. An empty slice is an explicitly empty list, which only
    /// `Mcp` can express (`[]`): both CLI interfaces refuse it.
    pub fn files(mut self, files: &[&str]) -> Self {
        self.files = Some(files.iter().map(|s| (*s).into()).collect());
        self
    }

    pub fn run(self) -> CheckResult {
        self.scute.run_check(&CheckInput::CodeSimilarity {
            source_dir: self.source_dir,
            files: self.files,
        })
    }
}

/// The result of listing available checks. Use its methods to assert on which checks are present.
pub struct ListChecksResult {
    pub(crate) _dir: TempDir,
    pub(crate) checks: Vec<String>,
}

impl ListChecksResult {
    pub fn expect_contains(&self, name: &str) -> &Self {
        assert!(
            self.checks.iter().any(|c| c == name),
            "expected check '{name}' in {:?}",
            self.checks
        );
        self
    }
}

/// How a check describes itself. Use its methods to assert on what it tells the caller.
pub struct CheckDescription {
    pub(crate) _dir: TempDir,
    pub(crate) text: String,
}

impl CheckDescription {
    pub fn expect_mentions(&self, expected: &str) -> &Self {
        assert!(
            self.text.contains(expected),
            "expected '{expected}' in:\n{}",
            self.text
        );
        self
    }
}

/// The result of running a check. Use its methods to assert on status, findings, and evidence.
pub struct CheckResult {
    pub(crate) _dir: TempDir,
    pub(crate) json: serde_json::Value,
    pub(crate) project_dir: PathBuf,
    pub(crate) exit_status: ExitStatus,
    pub(crate) debug_info: String,
}

impl CheckResult {
    pub fn expect_pass(&self) -> &Self {
        let summary = self.summary();
        assert!(
            summary["failed"] == 0
                && summary["errored"] == 0
                && summary["passed"].as_u64() > Some(0),
            "expected pass, got: {}",
            self.json
        );
        self.assert_exit_status(ExitStatus::Success);
        self
    }

    /// Nothing was evaluated, and the run still succeeded.
    pub fn expect_nothing_checked(&self) -> &Self {
        assert_eq!(
            self.summary(),
            &serde_json::json!({"evaluated": 0, "passed": 0, "warned": 0, "failed": 0, "errored": 0}),
            "expected nothing checked, got: {}",
            self.json
        );
        self.assert_exit_status(ExitStatus::Success);
        self
    }

    pub fn expect_warn(&self) -> &Self {
        self.assert_summary_nonzero("warned");
        self.assert_exit_status(ExitStatus::Success);
        self
    }

    pub fn expect_fail(&self) -> &Self {
        self.assert_summary_nonzero("failed");
        self.assert_exit_status(ExitStatus::Failure);
        self
    }

    pub fn expect_target(&self, expected: &str) -> &Self {
        assert_eq!(self.first_finding()["target"], expected);
        self
    }

    pub fn expect_target_contains(&self, substring: &str) -> &Self {
        let target = self.first_finding()["target"]
            .as_str()
            .expect("target should be a string");
        assert!(
            target.contains(substring),
            "expected target to contain '{substring}', got '{target}'"
        );
        self
    }

    pub fn expect_target_matches_dir(&self) -> &Self {
        let target = self.first_finding()["target"]
            .as_str()
            .expect("target should be a string");
        assert_eq!(
            std::path::Path::new(target).canonicalize().unwrap(),
            self.project_dir
        );
        self
    }

    pub fn expect_observed(&self, expected: u64) -> &Self {
        assert_eq!(self.first_finding()["measurement"]["observed"], expected);
        self
    }

    pub fn expect_evidence_rule(&self, index: usize, rule: &str) -> &Self {
        assert_eq!(self.first_finding()["evidence"][index]["rule"], rule);
        self
    }

    pub fn expect_evidence_count(&self, expected: usize) -> &Self {
        let evidence = self.first_finding()["evidence"]
            .as_array()
            .expect("evidence should be an array");
        assert_eq!(
            evidence.len(),
            expected,
            "expected {expected} evidence entries, got {}",
            evidence.len()
        );
        self
    }

    pub fn expect_evidence_found_contains(&self, index: usize, substring: &str) -> &Self {
        let found = self.first_finding()["evidence"][index]["found"]
            .as_str()
            .unwrap_or("");
        assert!(
            found.contains(substring),
            "expected evidence[{index}].found to contain {substring:?}, got {found:?}"
        );
        self
    }

    pub fn expect_evidence_has_expected(&self, index: usize) -> &Self {
        assert!(
            !self.first_finding()["evidence"][index]["expected"].is_null(),
            "expected evidence[{index}].expected to be present"
        );
        self
    }

    pub fn expect_evidence_expected_contains(&self, index: usize, substring: &str) -> &Self {
        let expected = self.first_finding()["evidence"][index]["expected"]
            .as_str()
            .unwrap_or("");
        assert!(
            expected.contains(substring),
            "expected evidence[{index}].expected to contain {substring:?}, got {expected:?}"
        );
        self
    }

    pub fn expect_evidence_no_expected(&self, index: usize) -> &Self {
        assert!(
            self.first_finding()["evidence"][index]
                .get("expected")
                .is_none(),
            "expected evidence[{index}].expected to be absent"
        );
        self
    }

    pub fn expect_finding_count(&self, expected: usize) -> &Self {
        assert_eq!(
            self.findings().len(),
            expected,
            "expected {expected} findings, got {}",
            self.findings().len()
        );
        self
    }

    pub fn expect_no_findings(&self) -> &Self {
        assert!(
            self.findings().is_empty(),
            "expected no findings, got: {:?}",
            self.findings()
        );
        self
    }

    /// One finding is an error with this code, for a target containing
    /// `target`, and the run exits with an error.
    pub fn expect_error_finding(&self, target: &str, code: &str) -> &Self {
        assert!(
            self.findings().iter().any(|finding| {
                finding["status"] == "error"
                    && finding["error"]["code"] == code
                    && finding["target"]
                        .as_str()
                        .is_some_and(|t| t.contains(target))
            }),
            "expected a {code} error finding for {target}, got: {}",
            self.json
        );
        self.assert_exit_status(ExitStatus::Error);
        self
    }

    pub fn expect_error(&self, code: &str) -> &Self {
        let error = &self.json["error"];
        assert_eq!(error["code"], code, "got: {}", self.json);
        assert!(
            error["message"].is_string(),
            "error.message should be present"
        );
        assert!(
            error["recovery"].is_string(),
            "error.recovery should be present"
        );
        self.assert_exit_status(ExitStatus::Error);
        self
    }

    pub fn debug(&self) -> &Self {
        eprintln!("{}", self.debug_info);
        eprintln!("json: {}", self.json);
        self
    }

    fn summary(&self) -> &serde_json::Value {
        &self.json["summary"]
    }

    fn findings(&self) -> &Vec<serde_json::Value> {
        self.json["findings"]
            .as_array()
            .expect("findings should be an array")
    }

    fn first_finding(&self) -> &serde_json::Value {
        self.findings()
            .first()
            .expect("expected at least one finding")
    }

    fn assert_summary_nonzero(&self, field: &str) {
        assert!(
            self.summary()[field].as_u64() > Some(0),
            "expected at least one {field}, got: {}",
            self.json
        );
    }

    fn assert_exit_status(&self, expected: ExitStatus) {
        assert_eq!(
            self.exit_status, expected,
            "expected {expected:?}, got {:?}:\n{}",
            self.exit_status, self.debug_info
        );
    }
}

pub fn target_bin(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::current_exe().expect("need current_exe for binary lookup");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn project_without_package_manager_holds_only_the_files_added() {
        let scute = Scute::new(Interface::Cli).source_file("notes.md", "# Notes");

        let dir = scute.project.build();

        assert_eq!(entries(dir.path()), [".git", ".gitignore", "notes.md"]);
    }

    #[test]
    #[should_panic(expected = "pick a package manager")]
    fn dependency_without_package_manager_refuses_to_build() {
        let scute = Scute::new(Interface::Cli).dependency("itoa", "1.0.0");

        scute.project.build();
    }

    #[test]
    #[should_panic(expected = "pick a package manager")]
    fn dev_dependency_without_package_manager_refuses_to_build() {
        let scute = Scute::new(Interface::Cli).dev_dependency("itoa", "1.0.0");

        scute.project.build();
    }

    #[test]
    #[should_panic(expected = "pick a package manager")]
    fn workspace_member_without_package_manager_refuses_to_build() {
        let project = TestProject::empty().member("apps/web", |member| member);

        project.build();
    }
}
