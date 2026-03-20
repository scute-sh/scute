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

fn cli_args(input: &CheckInput) -> Vec<String> {
    match input {
        CheckInput::CommitMessage { message } => {
            vec!["check".into(), "commit-message".into(), message.clone()]
        }
        CheckInput::DependencyFreshness { path } => {
            let mut args = vec!["check".into(), "dependency-freshness".into()];
            args.extend(path.iter().cloned());
            args
        }
        CheckInput::CodeComplexity { paths } => {
            let mut args = vec!["check".into(), "code-complexity".into()];
            args.extend(paths.iter().cloned());
            args
        }
        CheckInput::CodeSimilarity { source_dir, files } => {
            let mut args = vec!["check".into(), "code-similarity".into()];
            if let Some(dir) = source_dir {
                args.extend(["--source-dir".into(), dir.clone()]);
            }
            args.extend(files.iter().cloned());
            args
        }
    }
}

impl Backend for CliBackend {
    fn run_check(&self, dir: TempDir, working_dir: &Path, input: &CheckInput) -> CheckResult {
        let mut cmd = assert_cmd::Command::new(target_bin("scute"));
        cmd.current_dir(working_dir);
        let args = cli_args(input);
        if self.stdin {
            let stdin_input = args.last().expect("check must have args").clone();
            cmd.args(&args[..args.len() - 1]).write_stdin(stdin_input);
        } else {
            cmd.args(&args);
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
