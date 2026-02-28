use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
use scute_core::dependency_freshness::Level;
use scute_core::{CommitMessageDefinition, Status, Thresholds};
use serde::Deserialize;

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
    DependencyFreshness,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { check } => match check {
            Checks::CommitMessage { message } => {
                let message = resolve_message(message)?;
                let definition = load_definition(scute_core::CHECK_NAME)?;
                let result = scute_core::check_commit_message(&message, Some(&definition));
                let failed = result.status == Status::Fail;
                println!("{}", serde_json::to_string(&result)?);
                if failed {
                    std::process::exit(1);
                }
                Ok(())
            }
            Checks::DependencyFreshness => {
                let target = std::env::current_dir()?;
                let definition = load_freshness_definition()?;
                let result = scute_core::dependency_freshness::run(&target, &definition)?;
                let failed = result.status == Status::Fail;
                println!("{}", serde_json::to_string(&result)?);
                if failed {
                    std::process::exit(1);
                }
                Ok(())
            }
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
    let mut config: ScuteConfig = serde_yml::from_str(&contents)?;
    Ok(config.checks.remove(check_name))
}

fn load_freshness_definition() -> Result<scute_core::dependency_freshness::Definition> {
    let entry = load_check_entry(scute_core::dependency_freshness::CHECK_NAME)?;
    Ok(freshness_definition_from(entry))
}

fn freshness_definition_from(
    entry: Option<CheckEntry>,
) -> scute_core::dependency_freshness::Definition {
    use scute_core::dependency_freshness::Definition;

    let (level, thresholds) = match entry {
        Some(e) => {
            let level = e
                .config
                .and_then(|c| serde_json::from_value::<DependencyFreshnessConfig>(c).ok())
                .and_then(|c| c.level);
            (level, e.thresholds)
        }
        None => (None, None),
    };
    Definition {
        level: Some(level.unwrap_or_default()),
        thresholds,
    }
}

#[derive(Deserialize)]
struct DependencyFreshnessConfig {
    level: Option<Level>,
}

fn load_definition(check_name: &str) -> Result<CommitMessageDefinition> {
    let Some(entry) = load_check_entry(check_name)? else {
        return Ok(CommitMessageDefinition::default());
    };
    let types = entry
        .config
        .and_then(|c| serde_json::from_value::<CommitMessageConfig>(c).ok())
        .and_then(|c| c.types);
    Ok(CommitMessageDefinition {
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
    use scute_core::dependency_freshness::Level;

    #[test]
    fn freshness_config_reads_level_from_entry() {
        let entry = check_entry_from_yaml(
            r"
            config:
              level: minor
            ",
        );

        let definition = freshness_definition_from(Some(entry));

        assert_eq!(definition.level, Some(Level::Minor));
    }

    #[test]
    fn freshness_config_defaults_to_major_level() {
        let definition = freshness_definition_from(None);

        assert_eq!(definition.level, Some(Level::Major));
    }

    fn check_entry_from_yaml(yaml: &str) -> CheckEntry {
        serde_yml::from_str(yaml).unwrap()
    }
}
