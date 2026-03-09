use std::path::Path;

use tempfile::TempDir;

use crate::{Backend, CheckResult, ExitStatus, ListChecksResult, target_bin};

pub(crate) struct CliBackend {
    pub(crate) stdin: bool,
}

impl Backend for CliBackend {
    fn check(&self, dir: TempDir, working_dir: &Path, args: &[&str]) -> CheckResult {
        let mut cmd = assert_cmd::Command::new(target_bin("scute"));
        cmd.current_dir(working_dir);
        if self.stdin {
            let message = args.last().expect("CliStdin requires message in args");
            cmd.args(&args[..args.len() - 1])
                .write_stdin(message.to_string());
        } else {
            cmd.args(args);
        }
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
