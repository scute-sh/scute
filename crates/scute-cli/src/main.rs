use std::io::{IsTerminal, Read};

use anyhow::Result;
use clap::{Parser, Subcommand};
mod output;

use output::to_check_json;
use scute_core::{ExecutionError, commit_message, dependency_freshness};
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
            let cwd = std::env::current_dir()?;
            match check {
                Checks::List => {
                    let checks = [commit_message::CHECK_NAME, dependency_freshness::CHECK_NAME];
                    println!("{}", serde_json::to_string(&checks)?);
                    Ok(())
                }
                Checks::CommitMessage { message } => {
                    let message = resolve_message(message)?;
                    let definition = scute_config::load_commit_message_definition(&cwd)
                        .unwrap_or_else(|e| invalid_config(&e));
                    let outcome = commit_message::check(&message, &definition);
                    output(commit_message::CHECK_NAME, &outcome)
                }
                Checks::DependencyFreshness { path } => {
                    let target = match path {
                        Some(p) => p.into(),
                        None => cwd.clone(),
                    };
                    let definition = scute_config::load_freshness_definition(&cwd)
                        .unwrap_or_else(|e| invalid_config(&e));
                    let outcome = dependency_freshness::check(&target, &definition);
                    output(dependency_freshness::CHECK_NAME, &outcome)
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
                    "available checks: {}, {}",
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

fn output(check_name: &str, outcome: &scute_core::CheckOutcome) -> Result<()> {
    let json = to_check_json(check_name, outcome);
    println!("{}", serde_json::to_string(&json)?);
    if outcome.is_error() {
        std::process::exit(2);
    }
    if outcome.is_fail() {
        std::process::exit(1);
    }
    Ok(())
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
