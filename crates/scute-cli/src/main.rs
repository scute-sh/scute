use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
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
    config: Option<CommitMessageConfig>,
}

#[derive(Deserialize)]
struct CommitMessageConfig {
    types: Option<Vec<String>>,
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

fn load_definition(check_name: &str) -> Result<CommitMessageDefinition> {
    let path = Path::new(".scute.yml");
    if !path.exists() {
        return Ok(CommitMessageDefinition::default());
    }
    let contents = std::fs::read_to_string(path)?;
    let config: ScuteConfig = serde_yml::from_str(&contents)?;
    let Some(entry) = config.checks.get(check_name) else {
        return Ok(CommitMessageDefinition::default());
    };
    let types = entry.config.as_ref().and_then(|c| c.types.clone());
    Ok(CommitMessageDefinition {
        types,
        thresholds: entry.thresholds.clone(),
    })
}
