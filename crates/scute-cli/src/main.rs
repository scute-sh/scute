use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use scute_core::{
    CheckOutcome, Evidence, ExecutionError, Status, Thresholds, commit_message,
    dependency_freshness,
};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "scute")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Check {
        #[command(subcommand)]
        check: Checks,
    },
}

#[derive(Subcommand)]
enum Checks {
    CommitMessage { message: Option<String> },
    DependencyFreshness { path: Option<String> },
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

#[derive(Serialize)]
struct CheckOutcomeJson<'a> {
    check: &'a str,
    target: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation: Option<EvaluationJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a ExecutionError>,
}

#[derive(Serialize)]
struct EvaluationJson<'a> {
    status: &'a Status,
    measurement: MeasurementJson<'a>,
    evidence: &'a [Evidence],
}

#[derive(Serialize)]
struct MeasurementJson<'a> {
    observed: u64,
    thresholds: &'a Thresholds,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { check } => match check {
            Checks::CommitMessage { message } => {
                let message = resolve_message(message)?;
                let definition = load_commit_message_definition(commit_message::CHECK_NAME)?;
                let outcome = commit_message::check(&message, &definition);
                output(commit_message::CHECK_NAME, &outcome)
            }
            Checks::DependencyFreshness { path } => {
                let target = match path {
                    Some(ref p) => std::path::PathBuf::from(p)
                        .canonicalize()
                        .with_context(|| format!("can't resolve path: {p}"))?,
                    None => std::env::current_dir()?,
                };
                let definition = load_freshness_definition(dependency_freshness::CHECK_NAME)?;
                let outcome = dependency_freshness::check(&target, &definition);
                output(dependency_freshness::CHECK_NAME, &outcome)
            }
        },
    }
}

fn output(check_name: &str, outcome: &CheckOutcome) -> Result<()> {
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

fn to_check_json<'a>(check_name: &'a str, outcome: &'a CheckOutcome) -> CheckOutcomeJson<'a> {
    match &outcome.result {
        Ok(evaluation) => CheckOutcomeJson {
            check: check_name,
            target: &outcome.target,
            evaluation: Some(EvaluationJson {
                status: &evaluation.status,
                measurement: MeasurementJson {
                    observed: evaluation.observed,
                    thresholds: &evaluation.thresholds,
                },
                evidence: &evaluation.evidence,
            }),
            error: None,
        },
        Err(error) => CheckOutcomeJson {
            check: check_name,
            target: &outcome.target,
            evaluation: None,
            error: Some(error),
        },
    }
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

fn load_check_entry(check_name: &str) -> Result<Option<CheckEntry>> {
    let path = Path::new(".scute.yml");
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)?;
    let mut config: ScuteConfig =
        serde_yml::from_str(&contents).context("failed to parse .scute.yml")?;
    Ok(config.checks.remove(check_name))
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
struct DependencyFreshnessConfig {
    level: Option<dependency_freshness::Level>,
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

#[derive(Deserialize)]
struct CommitMessageConfig {
    types: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dependency_freshness::Level;

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
