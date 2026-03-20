use std::path::Path;

use tempfile::TempDir;

use crate::{Backend, CheckInput, CheckResult, ExitStatus, ListChecksResult, target_bin};

pub(crate) struct CliBackend {
    pub(crate) stdin: bool,
}

impl CliBackend {
    fn run_cmd(mut cmd: assert_cmd::Command, dir: TempDir, working_dir: &Path) -> CheckResult {
        let output = cmd.output().unwrap();
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        CheckResult {
            project_dir: working_dir.canonicalize().unwrap_or(working_dir.into()),
            json: serde_json::from_str(&stdout)
                .unwrap_or_else(|e| panic!("expected valid JSON in stdout: {e}\nstdout: {stdout}")),
            exit_status: match exit_code {
                0 => ExitStatus::Success,
                1 => ExitStatus::Failure,
                2 => ExitStatus::Error,
                other => panic!("unexpected exit code {other}"),
            },
            debug_info: format!("exit: {exit_code}\nstderr: {stderr}"),
            _dir: dir,
        }
    }
}

/// CLI args that are always present regardless of stdin mode.
fn cli_base_args(input: &CheckInput) -> Vec<String> {
    match input {
        CheckInput::CommitMessage { .. } => vec!["check".into(), "commit-message".into()],
        CheckInput::DependencyFreshness { path } => {
            let mut args = vec!["check".into(), "dependency-freshness".into()];
            args.extend(path.iter().cloned());
            args
        }
        CheckInput::CodeComplexity { .. } => vec!["check".into(), "code-complexity".into()],
        CheckInput::CodeSimilarity { source_dir, .. } => {
            let mut args = vec!["check".into(), "code-similarity".into()];
            if let Some(dir) = source_dir {
                args.extend(["--source-dir".into(), dir.clone()]);
            }
            args
        }
    }
}

/// Content that goes to stdin. Returns `None` for checks that don't support stdin.
fn stdin_content(input: &CheckInput) -> Option<String> {
    match input {
        CheckInput::CommitMessage { message } => Some(message.clone()),
        CheckInput::DependencyFreshness { .. } => None,
        CheckInput::CodeComplexity { paths } => Some(paths.join("\n")),
        CheckInput::CodeSimilarity { files, .. } => Some(files.join("\n")),
    }
}

/// Positional args that go on the command line in non-stdin mode.
fn cli_positional_args(input: &CheckInput) -> Vec<String> {
    match input {
        CheckInput::CommitMessage { message } => vec![message.clone()],
        CheckInput::DependencyFreshness { .. } => vec![],
        CheckInput::CodeComplexity { paths } => paths.clone(),
        CheckInput::CodeSimilarity { files, .. } => files.clone(),
    }
}

impl Backend for CliBackend {
    fn run_check(&self, dir: TempDir, working_dir: &Path, input: &CheckInput) -> CheckResult {
        let mut cmd = assert_cmd::Command::new(target_bin("scute"));
        cmd.current_dir(working_dir);
        cmd.args(cli_base_args(input));
        if self.stdin {
            if let Some(content) = stdin_content(input) {
                cmd.write_stdin(content);
            }
        } else {
            cmd.args(cli_positional_args(input));
        }
        Self::run_cmd(cmd, dir, working_dir)
    }

    fn list_checks(&self, dir: TempDir) -> ListChecksResult {
        let output = assert_cmd::Command::new(target_bin("scute"))
            .args(["check", "list"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let checks: Vec<String> = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("expected JSON array from `scute check list`: {e}\nstdout: {stdout}")
        });
        ListChecksResult { _dir: dir, checks }
    }
}
