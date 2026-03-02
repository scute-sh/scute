use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
use scute_core::{
    ExecutionError, Thresholds, commit_message, dependency_freshness, output::to_check_json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "scute")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Check {
        #[command(subcommand)]
        check: Checks,
    },
}

#[derive(Debug, Subcommand)]
enum Checks {
    CommitMessage { message: Option<String> },
    DependencyFreshness { path: Option<String> },
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
        Commands::Check { check } => match check {
            Checks::CommitMessage { message } => {
                let message = resolve_message(message)?;
                let definition = load_commit_message_definition(commit_message::CHECK_NAME)
                    .unwrap_or_else(|e| invalid_config(&e));
                let outcome = commit_message::check(&message, &definition);
                output(commit_message::CHECK_NAME, &outcome)
            }
            Checks::DependencyFreshness { path } => {
                let target = match path {
                    Some(p) => p.into(),
                    None => std::env::current_dir()?,
                };
                let definition = load_freshness_definition(dependency_freshness::CHECK_NAME)
                    .unwrap_or_else(|e| invalid_config(&e));
                let outcome = dependency_freshness::check(&target, &definition);
                output(dependency_freshness::CHECK_NAME, &outcome)
            }
        },
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
                recovery: "available checks: commit-message, dependency-freshness".into(),
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

fn invalid_config(err: &anyhow::Error) -> ! {
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

#[derive(Deserialize)]
struct ScuteConfig {
    #[serde(default)]
    checks: HashMap<String, CheckEntry>,
}

#[derive(Deserialize)]
struct CheckEntry {
    #[serde(default)]
    thresholds: Option<Thresholds>,
    #[serde(default)]
    config: Option<serde_json::Value>,
}

fn load_check_entry(check_name: &str) -> Result<Option<CheckEntry>> {
    let path = Path::new(".scute.yml");
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)?;
    let mut config: ScuteConfig = serde_yml::from_str(&contents)
        .map_err(|e| anyhow::anyhow!("{}", format_config_error(&e)))?;
    Ok(config.checks.remove(check_name))
}

fn format_config_error(err: &serde_yml::Error) -> String {
    let mut msg = strip_internal_types(&err.to_string());
    if !msg.contains("at line ")
        && let Some(loc) = err.location()
    {
        msg = format!("{msg} at line {} column {}", loc.line(), loc.column());
    }
    msg
}

fn strip_internal_types(msg: &str) -> String {
    for keyword in [", expected struct ", ", expected enum "] {
        if let Some(start) = msg.find(keyword) {
            let after_type_keyword = &msg[start + keyword.len()..];
            let type_name_end = after_type_keyword
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(after_type_keyword.len());
            return format!("{}{}", &msg[..start], &after_type_keyword[type_name_end..]);
        }
    }
    msg.to_string()
}

#[derive(Deserialize)]
struct DependencyFreshnessConfig {
    level: Option<dependency_freshness::Level>,
}

fn load_freshness_definition(check_name: &str) -> Result<dependency_freshness::Definition> {
    let entry = load_check_entry(check_name)?;
    freshness_definition_from(entry)
}

fn freshness_definition_from(
    entry: Option<CheckEntry>,
) -> Result<dependency_freshness::Definition> {
    use dependency_freshness::Definition;

    let Some(entry) = entry else {
        return Ok(Definition::default());
    };
    let level = match entry.config {
        Some(c) => serde_json::from_value::<DependencyFreshnessConfig>(c)?.level,
        None => None,
    };
    Ok(Definition {
        level: Some(level.unwrap_or_default()),
        thresholds: entry.thresholds,
    })
}

#[derive(Deserialize)]
struct CommitMessageConfig {
    types: Option<Vec<String>>,
}

fn load_commit_message_definition(check_name: &str) -> Result<commit_message::Definition> {
    use commit_message::Definition;

    let Some(entry) = load_check_entry(check_name)? else {
        return Ok(Definition::default());
    };
    let types = match entry.config {
        Some(c) => serde_json::from_value::<CommitMessageConfig>(c)?.types,
        None => None,
    };
    Ok(Definition {
        types,
        thresholds: entry.thresholds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dependency_freshness::Level;

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

    #[test]
    fn freshness_config_reads_level_from_entry() {
        let entry = check_entry_from_yaml(
            r"
            config:
              level: minor
            ",
        );

        let definition = freshness_definition_from(Some(entry)).unwrap();

        assert_eq!(definition.level, Some(Level::Minor));
    }

    #[test]
    fn no_entry_returns_default_definition() {
        let definition = freshness_definition_from(None).unwrap();

        assert_eq!(definition.level, None);
        assert_eq!(definition.thresholds, None);
    }

    #[test]
    fn freshness_config_without_level_defaults_to_major() {
        let entry = check_entry_from_yaml(
            r"
            thresholds:
              fail: 5
            ",
        );

        let definition = freshness_definition_from(Some(entry)).unwrap();

        assert_eq!(definition.level, Some(Level::Major));
        assert_eq!(
            definition.thresholds,
            Some(Thresholds {
                warn: None,
                fail: Some(5),
            })
        );
    }

    #[test]
    fn freshness_config_rejects_invalid_level() {
        let entry = check_entry_from_yaml(
            r"
            config:
              level: bananas
            ",
        );

        assert!(freshness_definition_from(Some(entry)).is_err());
    }

    fn check_entry_from_yaml(yaml: &str) -> CheckEntry {
        serde_yml::from_str(yaml).unwrap()
    }
}
