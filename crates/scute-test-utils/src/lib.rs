#![allow(
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::return_self_not_must_use
)]

mod cli;
pub mod mcp;
mod project;

use std::path::Path;

use cli::CliBackend;
use mcp::McpBackend;
pub use project::TestProject;
use tempfile::TempDir;

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
    fn check(&self, dir: TempDir, working_dir: &Path, args: &[&str]) -> Box<dyn CheckResult>;
    fn list_checks(&self, dir: TempDir) -> Box<dyn ListChecksResult>;
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

    pub fn list_checks(self) -> Box<dyn ListChecksResult> {
        let dir = self.project.build();
        self.backend.list_checks(dir)
    }

    pub fn check(self, args: &[&str]) -> Box<dyn CheckResult> {
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

pub trait ListChecksResult {
    fn expect_contains(&self, name: &str) -> &dyn ListChecksResult;
}

pub trait CheckResult {
    fn expect_pass(&self) -> &dyn CheckResult;
    fn expect_warn(&self) -> &dyn CheckResult;
    fn expect_fail(&self) -> &dyn CheckResult;
    fn expect_target(&self, expected: &str) -> &dyn CheckResult;
    fn expect_target_contains(&self, substring: &str) -> &dyn CheckResult;
    fn expect_target_matches_dir(&self) -> &dyn CheckResult;
    fn expect_observed(&self, expected: u64) -> &dyn CheckResult;
    fn expect_evidence_rule(&self, index: usize, rule: &str) -> &dyn CheckResult;
    fn expect_evidence_has_expected(&self, index: usize) -> &dyn CheckResult;
    fn expect_evidence_no_expected(&self, index: usize) -> &dyn CheckResult;
    fn expect_finding_count(&self, expected: usize) -> &dyn CheckResult;
    fn expect_no_findings(&self) -> &dyn CheckResult;
    fn expect_error(&self, code: &str) -> &dyn CheckResult;
    fn debug(&self) -> &dyn CheckResult;
}

fn target_bin(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::current_exe().expect("need current_exe for binary lookup");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
