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
pub use project::TestProject;
use tempfile::TempDir;

/// How the check process terminated, from the interface's perspective.
///
/// CLI maps exit codes: 0 → Success, 1 → Failure, 2 → Error.
/// MCP maps `isError` + JSON shape: no error → Success, isError + findings → Failure,
/// isError + error object → Error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ExitStatus {
    Success,
    Failure,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub enum Interface {
    Cli,
    CliStdin,
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

trait Backend {
    fn check(&self, dir: TempDir, working_dir: &Path, args: &[&str]) -> CheckResult;
    fn list_checks(&self, dir: TempDir) -> ListChecksResult;
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
            backend: Box::new(CliBackend { stdin: false }),
            project: TestProject::cargo(),
            cwd: None,
        }
    }

    pub fn cli_stdin() -> Self {
        Self {
            backend: Box::new(CliBackend { stdin: true }),
            project: TestProject::cargo(),
            cwd: None,
        }
    }

    pub fn mcp() -> Self {
        Self {
            backend: Box::new(McpBackend),
            project: TestProject::cargo(),
            cwd: None,
        }
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

    pub fn list_checks(self) -> ListChecksResult {
        let dir = self.project.build();
        self.backend.list_checks(dir)
    }

    pub fn check(self, args: &[&str]) -> CheckResult {
        let mut full_args = vec!["check"];
        full_args.extend_from_slice(args);
        let dir = self.project.build();
        let working_dir = match &self.cwd {
            Some(subdir) => {
                let path = dir.path().join(subdir);
                std::fs::create_dir_all(&path).expect("failed to create cwd subdir");
                path
            }
            None => dir.path().to_path_buf(),
        };
        self.backend.check(dir, &working_dir, &full_args)
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
