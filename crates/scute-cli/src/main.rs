use std::io::{BufRead, IsTerminal, Read};
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
mod output;

use output::CheckReportJson;
use scute_config::ScuteConfig;
use scute_core::report::CheckReport;
use scute_core::{
    ExecutionError, code_complexity, code_similarity, commit_message, dependency_freshness,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "scute",
    about = "Define the boundaries. Let your code evolve freely within them."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run a fitness check
    Check {
        #[command(subcommand)]
        check: Checks,
    },
    /// Serve checks to coding agents
    Mcp,
}

#[derive(Debug, Subcommand)]
enum Checks {
    /// List available checks
    List,
    /// Validate a commit message
    CommitMessage {
        /// Commit message to check
        message: Option<String>,
    },
    /// Find code duplication
    CodeSimilarity {
        /// Directory to scan for source files (defaults to working directory)
        #[arg(long)]
        source_dir: Option<PathBuf>,
        /// Files to focus on (only report clones involving these). Reads from stdin if piped.
        files: Vec<PathBuf>,
    },
    /// Measure code complexity of functions
    CodeComplexity {
        /// Files or directories to check. Reads from stdin if piped. Defaults to working directory.
        paths: Vec<PathBuf>,
    },
    /// Find outdated dependencies
    DependencyFreshness {
        /// Path to the project directory (defaults to working directory)
        path: Option<String>,
    },
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if err.use_stderr() {
                engine_error(&classify_clap_error(&err));
            }
            err.exit();
        }
    };

    match run(cli) {
        Ok(()) => {}
        Err(err) => engine_error(&ExecutionError {
            code: "unhandled_error".into(),
            message: format!("{err}"),
            recovery: "please report this issue".into(),
        }),
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Mcp => scute_mcp::run().map_err(|e| anyhow::anyhow!(e)),
        Commands::Check { check } => {
            let project_root = project_root();
            let config = ScuteConfig::load(&project_root).unwrap_or_else(|e| invalid_config(&e));
            match check {
                Checks::List => {
                    let checks = [
                        code_similarity::CHECK_NAME,
                        code_complexity::CHECK_NAME,
                        commit_message::CHECK_NAME,
                        dependency_freshness::CHECK_NAME,
                    ];
                    println!("{}", serde_json::to_string(&checks)?);
                    Ok(())
                }
                Checks::CodeComplexity { paths } => {
                    let paths = resolve_paths(paths, &project_root);
                    let definition: code_complexity::Definition = config
                        .definition(code_complexity::CHECK_NAME)
                        .unwrap_or_else(|e| invalid_config(&e));
                    let result = code_complexity::check(&paths, &definition);
                    output(&CheckReport::new(code_complexity::CHECK_NAME, result))
                }
                Checks::CodeSimilarity { source_dir, files } => run_source_check(
                    &config,
                    &project_root,
                    source_dir,
                    files,
                    code_similarity::CHECK_NAME,
                    code_similarity::check,
                ),
                Checks::CommitMessage { message } => {
                    let message = resolve_message(message)?;
                    let definition: commit_message::Definition = config
                        .definition(commit_message::CHECK_NAME)
                        .unwrap_or_else(|e| invalid_config(&e));
                    let result = commit_message::check(&message, &definition);
                    output(&CheckReport::new(commit_message::CHECK_NAME, result))
                }
                Checks::DependencyFreshness { path } => {
                    let target = resolve_target_path(path);
                    let definition: dependency_freshness::Definition = config
                        .definition(dependency_freshness::CHECK_NAME)
                        .unwrap_or_else(|e| invalid_config(&e));
                    let result = dependency_freshness::check(&target, &definition);
                    output(&CheckReport::new(dependency_freshness::CHECK_NAME, result))
                }
            }
        }
    }
}

fn classify_clap_error(err: &clap::Error) -> ExecutionError {
    use clap::error::ErrorKind;
    match err.kind() {
        ErrorKind::InvalidSubcommand if is_check_level(err) => {
            let name = err
                .get(clap::error::ContextKind::InvalidSubcommand)
                .map(std::string::ToString::to_string)
                .unwrap_or_default();
            ExecutionError {
                code: "unknown_check".into(),
                message: format!("unknown check: {name}"),
                recovery: format!(
                    "available checks: {}, {}, {}, {}",
                    code_similarity::CHECK_NAME,
                    code_complexity::CHECK_NAME,
                    commit_message::CHECK_NAME,
                    dependency_freshness::CHECK_NAME
                ),
            }
        }
        _ => ExecutionError {
            code: "invalid_usage".into(),
            message: "missing or invalid arguments".into(),
            recovery: "run scute --help for usage".into(),
        },
    }
}

fn is_check_level(err: &clap::Error) -> bool {
    err.get(clap::error::ContextKind::Usage)
        .is_some_and(|usage| usage.to_string().contains("scute check"))
}

fn invalid_config(err: &scute_config::ConfigError) -> ! {
    engine_error(&ExecutionError {
        code: "invalid_config".into(),
        message: format!("{err}"),
        recovery: "check your .scute.yml syntax".into(),
    })
}

#[derive(Serialize)]
struct EngineErrorJson<'a> {
    error: &'a ExecutionError,
}

fn engine_error(error: &ExecutionError) -> ! {
    let json = EngineErrorJson { error };
    println!(
        "{}",
        serde_json::to_string(&json).expect("engine error serializes")
    );
    std::process::exit(2);
}

fn output(report: &CheckReport) -> Result<()> {
    let json = CheckReportJson::from(report);
    println!("{}", serde_json::to_string(&json)?);
    if report.has_errors() {
        std::process::exit(2);
    }
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}

fn project_root() -> PathBuf {
    std::env::current_dir().expect("working directory accessible")
}

fn run_source_check<D: Default + serde::de::DeserializeOwned>(
    config: &ScuteConfig,
    project_root: &Path,
    source_dir: Option<PathBuf>,
    files: Vec<PathBuf>,
    check_name: &str,
    execute: impl FnOnce(&Path, &[PathBuf], &D) -> Result<Vec<scute_core::Evaluation>, ExecutionError>,
) -> Result<()> {
    let source_dir = source_dir.unwrap_or_else(|| project_root.to_path_buf());
    let focus_files = resolve_focus_files(files);
    let definition: D = config
        .definition(check_name)
        .unwrap_or_else(|e| invalid_config(&e));
    let result = execute(&source_dir, &focus_files, &definition);
    output(&CheckReport::new(check_name, result))
}

fn resolve_target_path(path: Option<String>) -> PathBuf {
    path.map_or_else(project_root, PathBuf::from)
}

fn resolve_paths(paths: Vec<PathBuf>, default_dir: &Path) -> Vec<PathBuf> {
    let paths = read_from_stdin_if_empty(paths);
    if paths.is_empty() {
        vec![default_dir.to_path_buf()]
    } else {
        paths
    }
}

fn resolve_focus_files(files: Vec<PathBuf>) -> Vec<PathBuf> {
    read_from_stdin_if_empty(files)
}

fn read_from_stdin_if_empty(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    if !paths.is_empty() {
        return paths;
    }
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Vec::new();
    }
    stdin
        .lock()
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn resolve_message(arg: Option<String>) -> Result<String> {
    if let Some(message) = arg {
        return Ok(message);
    }
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(String::new());
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_check_name_classifies_as_unknown_check() {
        let err = Cli::try_parse_from(["scute", "check", "does-not-exist"]).unwrap_err();

        let error = classify_clap_error(&err);

        assert_eq!(error.code, "unknown_check");
    }

    #[test]
    fn missing_check_subcommand_classifies_as_invalid_usage() {
        let err = Cli::try_parse_from(["scute", "check"]).unwrap_err();

        let error = classify_clap_error(&err);

        assert_eq!(error.code, "invalid_usage");
    }

    #[test]
    fn missing_top_level_subcommand_classifies_as_invalid_usage() {
        let err = Cli::try_parse_from(["scute", "commit-message", "feat: test"]).unwrap_err();

        let error = classify_clap_error(&err);

        assert_eq!(error.code, "invalid_usage");
    }
}
