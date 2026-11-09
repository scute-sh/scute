use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::{Backend, CheckInput, CheckResult, ExitStatus, ListChecksResult, target_bin};

pub(crate) struct CliBackend {
    pub(crate) via_stdin: bool,
}

/// How the CLI gets run: its arguments, and what gets piped into stdin.
/// `None` runs it with stdin closed, the way a terminal-less shell would.
#[derive(Debug, PartialEq)]
struct Invocation {
    args: Vec<String>,
    stdin: Option<String>,
}

impl Invocation {
    fn run(&self, working_dir: &Path) -> Output {
        let mut child = Command::new(target_bin("scute"))
            .args(&self.args)
            .current_dir(working_dir)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn scute");
        if let Some(content) = &self.stdin {
            child
                .stdin
                .take()
                .expect("stdin is piped")
                .write_all(content.as_bytes())
                .expect("failed to write stdin");
        }
        child.wait_with_output().expect("failed to wait for scute")
    }
}

impl CliBackend {
    fn check_result(output: &Output, dir: TempDir, working_dir: &Path) -> CheckResult {
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

    fn invocation(&self, input: &CheckInput) -> Invocation {
        let empty_path_list = given_paths(input).is_some_and(Vec::is_empty);
        assert!(
            !empty_path_list,
            "only Interface::Mcp can pass an empty path list: the CLI reads no paths as no input"
        );
        let mut args = cli_base_args(input);
        if self.via_stdin {
            return Invocation {
                args,
                stdin: Some(stdin_content(input)),
            };
        }
        args.extend(cli_positional_args(input));
        Invocation { args, stdin: None }
    }
}

/// The paths a path-taking check was given, or `None` when it got no input.
fn given_paths(input: &CheckInput) -> Option<&Vec<String>> {
    match input {
        CheckInput::CodeComplexity { paths } => paths.as_ref(),
        CheckInput::CodeSimilarity { files, .. } => files.as_ref(),
        CheckInput::CommitMessage { .. } | CheckInput::DependencyFreshness { .. } => None,
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

/// Content to pipe into stdin. No input pipes nothing at all.
fn stdin_content(input: &CheckInput) -> String {
    match input {
        CheckInput::CommitMessage { message } => message.clone(),
        _ => given_paths(input)
            .map(|paths| paths.join("\n"))
            .unwrap_or_default(),
    }
}

/// Positional args that go on the command line in non-stdin mode.
fn cli_positional_args(input: &CheckInput) -> Vec<String> {
    match input {
        CheckInput::CommitMessage { message } => vec![message.clone()],
        _ => given_paths(input).cloned().unwrap_or_default(),
    }
}

impl Backend for CliBackend {
    fn run_check(&self, dir: TempDir, working_dir: &Path, input: &CheckInput) -> CheckResult {
        let output = self.invocation(input).run(working_dir);
        Self::check_result(&output, dir, working_dir)
    }

    fn list_checks(&self, dir: TempDir) -> ListChecksResult {
        let output = Invocation {
            args: vec!["check".into(), "list".into()],
            stdin: None,
        }
        .run(dir.path());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let checks: Vec<String> = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("expected JSON array from `scute check list`: {e}\nstdout: {stdout}")
        });
        ListChecksResult { _dir: dir, checks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "only Interface::Mcp can pass an empty path list")]
    fn args_driver_refuses_empty_path_list() {
        let driver = CliBackend { via_stdin: false };

        driver.invocation(&CheckInput::CodeComplexity {
            paths: Some(vec![]),
        });
    }

    #[test]
    #[should_panic(expected = "only Interface::Mcp can pass an empty path list")]
    fn args_driver_refuses_empty_focus_file_list() {
        let driver = CliBackend { via_stdin: false };

        driver.invocation(&CheckInput::CodeSimilarity {
            source_dir: None,
            files: Some(vec![]),
        });
    }

    #[test]
    fn args_driver_passes_paths_as_arguments_with_stdin_closed() {
        let driver = CliBackend { via_stdin: false };

        let invocation = driver.invocation(&CheckInput::CodeComplexity {
            paths: Some(vec!["a.rs".into()]),
        });

        assert_eq!(
            invocation,
            Invocation {
                args: vec!["check".into(), "code-complexity".into(), "a.rs".into()],
                stdin: None,
            }
        );
    }

    #[test]
    fn stdin_driver_pipes_paths_one_per_line() {
        let driver = CliBackend { via_stdin: true };

        let invocation = driver.invocation(&CheckInput::CodeComplexity {
            paths: Some(vec!["a.rs".into(), "b.rs".into()]),
        });

        assert_eq!(
            invocation,
            Invocation {
                args: vec!["check".into(), "code-complexity".into()],
                stdin: Some("a.rs\nb.rs".into()),
            }
        );
    }

    #[test]
    fn stdin_driver_runs_similarity_without_focus_files_as_empty_pipe() {
        let driver = CliBackend { via_stdin: true };

        let invocation = driver.invocation(&CheckInput::CodeSimilarity {
            source_dir: None,
            files: None,
        });

        assert_eq!(
            invocation,
            Invocation {
                args: vec!["check".into(), "code-similarity".into()],
                stdin: Some(String::new()),
            }
        );
    }

    #[test]
    fn stdin_driver_runs_without_input_as_empty_pipe() {
        let driver = CliBackend { via_stdin: true };

        let invocation = driver.invocation(&CheckInput::CodeComplexity { paths: None });

        assert_eq!(
            invocation,
            Invocation {
                args: vec!["check".into(), "code-complexity".into()],
                stdin: Some(String::new()),
            }
        );
    }

    #[test]
    #[should_panic(expected = "only Interface::Mcp can pass an empty path list")]
    fn stdin_driver_refuses_empty_path_list() {
        let driver = CliBackend { via_stdin: true };

        driver.invocation(&CheckInput::CodeComplexity {
            paths: Some(vec![]),
        });
    }

    #[test]
    fn args_driver_runs_without_input_with_no_paths_and_stdin_closed() {
        let driver = CliBackend { via_stdin: false };

        let invocation = driver.invocation(&CheckInput::CodeComplexity { paths: None });

        assert_eq!(
            invocation,
            Invocation {
                args: vec!["check".into(), "code-complexity".into()],
                stdin: None,
            }
        );
    }
}
