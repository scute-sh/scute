#![allow(clippy::missing_errors_doc)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

const CONFIG_FILE: &str = ".scute.yml";

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

#[derive(Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    checks: HashMap<String, serde_yml::Value>,
}

pub struct ScuteConfig {
    checks: HashMap<String, serde_yml::Value>,
}

impl ScuteConfig {
    pub fn load(dir: &Path) -> Result<Self, ConfigError> {
        let Some(path) = find_config_file(dir) else {
            return Ok(Self {
                checks: HashMap::new(),
            });
        };
        let contents = std::fs::read_to_string(&path).map_err(ConfigError::Io)?;
        let raw: RawConfig = serde_yml::from_str(&contents)
            .map_err(|e| ConfigError::Parse(format_config_error(&e)))?;
        Ok(Self { checks: raw.checks })
    }

    pub fn definition<D: Default + DeserializeOwned>(&self, name: &str) -> Result<D, ConfigError> {
        match self.checks.get(name) {
            Some(value) => serde_yml::from_value(value.clone())
                .map_err(|e| ConfigError::InvalidCheckConfig(format_config_error(&e))),
            None => Ok(D::default()),
        }
    }
}

fn find_config_file(start: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir();
    for dir in start.ancestors() {
        let candidate = dir.join(CONFIG_FILE);
        if candidate.exists() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            return None;
        }
        if home.as_deref() == Some(dir) {
            return None;
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use scute_core::{Thresholds, code_similarity, commit_message, dependency_freshness};

    fn config_from_yaml(yaml: &str) -> ScuteConfig {
        let checks: HashMap<String, serde_yml::Value> = serde_yml::from_str(yaml).unwrap();
        ScuteConfig { checks }
    }

    fn definition<D: Default + DeserializeOwned>(yaml: &str, check: &str) -> D {
        config_from_yaml(yaml).definition(check).unwrap()
    }

    #[test]
    fn no_entry_returns_default_freshness_definition() {
        let def: dependency_freshness::Definition =
            definition("{}", dependency_freshness::CHECK_NAME);

        assert_eq!(def.level, None);
        assert_eq!(def.thresholds, None);
    }

    #[test]
    fn freshness_reads_level() {
        let def: dependency_freshness::Definition = definition(
            r"
            dependency-freshness:
              level: minor
            ",
            dependency_freshness::CHECK_NAME,
        );

        assert_eq!(def.level, Some(dependency_freshness::Level::Minor));
    }

    #[test]
    fn freshness_reads_thresholds() {
        let def: dependency_freshness::Definition = definition(
            r"
            dependency-freshness:
              thresholds:
                fail: 5
            ",
            dependency_freshness::CHECK_NAME,
        );

        assert_eq!(
            def.thresholds,
            Some(Thresholds {
                warn: None,
                fail: Some(5),
            })
        );
    }

    #[test]
    fn freshness_rejects_invalid_level() {
        let config = config_from_yaml(
            r"
            dependency-freshness:
              level: bananas
            ",
        );

        assert!(
            config
                .definition::<dependency_freshness::Definition>(dependency_freshness::CHECK_NAME)
                .is_err()
        );
    }

    #[test]
    fn no_entry_returns_default_code_similarity_definition() {
        let def: code_similarity::Definition = definition("{}", code_similarity::CHECK_NAME);

        assert_eq!(def.min_tokens, None);
        assert_eq!(def.thresholds, None);
    }

    #[test]
    fn code_similarity_reads_min_tokens_with_kebab_case() {
        let def: code_similarity::Definition = definition(
            r"
            code-similarity:
              min-tokens: 10
            ",
            code_similarity::CHECK_NAME,
        );

        assert_eq!(def.min_tokens, Some(10));
    }

    #[test]
    fn code_similarity_reads_min_tokens_with_snake_case() {
        let def: code_similarity::Definition = definition(
            r"
            code-similarity:
              min_tokens: 10
            ",
            code_similarity::CHECK_NAME,
        );

        assert_eq!(def.min_tokens, Some(10));
    }

    #[test]
    fn code_similarity_reads_thresholds() {
        let def: code_similarity::Definition = definition(
            r"
            code-similarity:
              thresholds:
                warn: 20
                fail: 50
            ",
            code_similarity::CHECK_NAME,
        );

        assert_eq!(
            def.thresholds,
            Some(Thresholds {
                warn: Some(20),
                fail: Some(50),
            })
        );
    }

    #[test]
    fn code_similarity_reads_test_thresholds_with_kebab_case() {
        let def: code_similarity::Definition = definition(
            r"
            code-similarity:
              test-thresholds:
                warn: 100
                fail: 200
            ",
            code_similarity::CHECK_NAME,
        );

        assert_eq!(
            def.test_thresholds,
            Some(Thresholds {
                warn: Some(100),
                fail: Some(200),
            })
        );
    }

    #[test]
    fn rejects_unknown_fields_in_check_definition() {
        let config = config_from_yaml(
            r"
            code-similarity:
              config:
                min-tokens: 25
            ",
        );

        assert!(
            config
                .definition::<code_similarity::Definition>(code_similarity::CHECK_NAME)
                .is_err()
        );
    }

    #[test]
    fn commit_message_reads_types() {
        let def: commit_message::Definition = definition(
            r"
            commit-message:
              types: [hotfix, deploy]
            ",
            commit_message::CHECK_NAME,
        );

        assert_eq!(def.types, Some(vec!["hotfix".into(), "deploy".into()]));
    }

    mod find_config_file {
        use super::super::*;

        #[test]
        fn finds_config_in_start_directory() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(CONFIG_FILE), "").unwrap();

            assert_eq!(
                find_config_file(dir.path()),
                Some(dir.path().join(CONFIG_FILE))
            );
        }

        #[test]
        fn finds_config_in_parent_directory() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(CONFIG_FILE), "").unwrap();
            let child = dir.path().join("sub");
            std::fs::create_dir(&child).unwrap();

            assert_eq!(find_config_file(&child), Some(dir.path().join(CONFIG_FILE)));
        }

        #[test]
        fn stops_at_git_boundary() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(CONFIG_FILE), "").unwrap();
            let project = dir.path().join("project");
            std::fs::create_dir(&project).unwrap();
            std::fs::create_dir(project.join(".git")).unwrap();
            let child = project.join("sub");
            std::fs::create_dir(&child).unwrap();

            assert_eq!(find_config_file(&child), None);
        }

        #[test]
        fn returns_none_when_no_config_found() {
            let dir = tempfile::tempdir().unwrap();
            let child = dir.path().join("a/b/c");
            std::fs::create_dir_all(&child).unwrap();

            assert_eq!(find_config_file(&child), None);
        }
    }
}
