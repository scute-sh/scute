#![allow(
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::return_self_not_must_use
)]

mod cli;
mod mcp;
mod project;

use cli::CliBackend;
use mcp::McpBackend;
pub use project::TestProject;
use tempfile::TempDir;

trait Backend {
    fn check(&self, dir: TempDir, args: &[&str]) -> Box<dyn CheckResult>;
    fn list_checks(&self, dir: TempDir) -> Box<dyn ListChecksResult>;
}

pub struct Scute {
    backend: Box<dyn Backend>,
    project: TestProject,
}

impl Scute {
    pub fn cli() -> Self {
        Self {
            backend: Box::new(CliBackend { stdin: false }),
            project: TestProject::cargo(),
        }
    }

    pub fn cli_stdin() -> Self {
        Self {
            backend: Box::new(CliBackend { stdin: true }),
            project: TestProject::cargo(),
        }
    }

    pub fn mcp() -> Self {
        Self {
            backend: Box::new(McpBackend),
            project: TestProject::cargo(),
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

    pub fn scute_config(mut self, yaml: &str) -> Self {
        self.project = self.project.scute_config(yaml);
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
        self.backend.check(dir, &full_args)
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
    fn expect_target_matches_dir(&self) -> &dyn CheckResult;
    fn expect_observed(&self, expected: u64) -> &dyn CheckResult;
    fn expect_evidence_rule(&self, index: usize, rule: &str) -> &dyn CheckResult;
    fn expect_evidence_has_expected(&self, index: usize) -> &dyn CheckResult;
    fn expect_evidence_no_expected(&self, index: usize) -> &dyn CheckResult;
    fn expect_no_evidences(&self) -> &dyn CheckResult;
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
