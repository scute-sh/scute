#![allow(clippy::missing_errors_doc)]

use std::collections::HashMap;
use std::path::Path;

use scute_core::{Thresholds, code_similarity, commit_message, dependency_freshness};
use serde::Deserialize;

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

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(String),
    InvalidCheckConfig(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(msg) | Self::InvalidCheckConfig(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn load_check_entry(dir: &Path, check_name: &str) -> Result<Option<CheckEntry>, ConfigError> {
    let path = dir.join(".scute.yml");
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path).map_err(ConfigError::Io)?;
    let mut config: ScuteConfig =
        serde_yml::from_str(&contents).map_err(|e| ConfigError::Parse(format_config_error(&e)))?;
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

pub fn load_code_similarity_definition(
    dir: &Path,
) -> Result<code_similarity::Definition, ConfigError> {
    let entry = load_check_entry(dir, code_similarity::CHECK_NAME)?;
    code_similarity_definition_from(entry)
}

fn code_similarity_definition_from(
    entry: Option<CheckEntry>,
) -> Result<code_similarity::Definition, ConfigError> {
    let Some(entry) = entry else {
        return Ok(code_similarity::Definition::default());
    };
    let min_tokens = match entry.config {
        Some(c) => {
            serde_json::from_value::<CodeSimilarityConfig>(c)
                .map_err(|e| ConfigError::InvalidCheckConfig(e.to_string()))?
                .min_tokens
        }
        None => None,
    };
    Ok(code_similarity::Definition {
        min_tokens,
        thresholds: entry.thresholds,
    })
}

#[derive(Deserialize)]
struct CodeSimilarityConfig {
    #[serde(alias = "min-tokens")]
    min_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct DependencyFreshnessConfig {
    level: Option<dependency_freshness::Level>,
}

pub fn load_freshness_definition(
    dir: &Path,
) -> Result<dependency_freshness::Definition, ConfigError> {
    let entry = load_check_entry(dir, dependency_freshness::CHECK_NAME)?;
    freshness_definition_from(entry)
}

fn freshness_definition_from(
    entry: Option<CheckEntry>,
) -> Result<dependency_freshness::Definition, ConfigError> {
    let Some(entry) = entry else {
        return Ok(dependency_freshness::Definition::default());
    };
    let level = match entry.config {
        Some(c) => {
            serde_json::from_value::<DependencyFreshnessConfig>(c)
                .map_err(|e| ConfigError::InvalidCheckConfig(e.to_string()))?
                .level
        }
        None => None,
    };
    Ok(dependency_freshness::Definition {
        level: Some(level.unwrap_or_default()),
        thresholds: entry.thresholds,
    })
}

#[derive(Deserialize)]
struct CommitMessageConfig {
    types: Option<Vec<String>>,
}

pub fn load_commit_message_definition(
    dir: &Path,
) -> Result<commit_message::Definition, ConfigError> {
    let entry = load_check_entry(dir, commit_message::CHECK_NAME)?;
    let Some(entry) = entry else {
        return Ok(commit_message::Definition::default());
    };
    let types = match entry.config {
        Some(c) => {
            serde_json::from_value::<CommitMessageConfig>(c)
                .map_err(|e| ConfigError::InvalidCheckConfig(e.to_string()))?
                .types
        }
        None => None,
    };
    Ok(commit_message::Definition {
        types,
        thresholds: entry.thresholds,
    })
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
    fn no_entry_returns_default_freshness_definition() {
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

    #[test]
    fn no_entry_returns_default_code_similarity_definition() {
        let definition = code_similarity_definition_from(None).unwrap();

        assert_eq!(definition.min_tokens, None);
        assert_eq!(definition.thresholds, None);
    }

    #[test]
    fn code_similarity_config_reads_min_tokens_from_entry() {
        let entry = check_entry_from_yaml(
            r"
            config:
              min-tokens: 10
            ",
        );

        let definition = code_similarity_definition_from(Some(entry)).unwrap();

        assert_eq!(definition.min_tokens, Some(10));
    }

    #[test]
    fn code_similarity_config_reads_thresholds_from_entry() {
        let entry = check_entry_from_yaml(
            r"
            thresholds:
              warn: 20
              fail: 50
            ",
        );

        let definition = code_similarity_definition_from(Some(entry)).unwrap();

        assert_eq!(
            definition.thresholds,
            Some(Thresholds {
                warn: Some(20),
                fail: Some(50),
            })
        );
    }

    fn check_entry_from_yaml(yaml: &str) -> CheckEntry {
        serde_yml::from_str(yaml).unwrap()
    }
}
