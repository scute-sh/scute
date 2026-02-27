use std::io::Read;

use anyhow::Result;
use clap::{Parser, Subcommand};
use scute_core::Status;

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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { check } => match check {
            Checks::CommitMessage { message } => {
                let message = resolve_message(message)?;
                let result = scute_core::check_commit_message(&message);
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
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}
