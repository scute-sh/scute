use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Thresholds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail: Option<u64>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct CheckResult {
    pub check: String,
    pub target: String,
    pub status: Status,
    pub observed: u64,
    pub expected: Thresholds,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Evidence {
    pub rule: String,
    pub found: String,
}

impl Evidence {
    fn new(rule: &str, found: &str) -> Self {
        Self {
            rule: rule.into(),
            found: found.into(),
        }
    }
}

const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

#[must_use]
pub fn check_commit_message(message: &str) -> CheckResult {
    let subject = message.lines().next().unwrap_or("");
    let evidence = validate_subject(subject);
    let observed = u64::from(!evidence.is_empty());

    CheckResult {
        check: "commit-message".into(),
        target: subject.into(),
        status: if observed > 0 {
            Status::Fail
        } else {
            Status::Pass
        },
        observed,
        expected: Thresholds {
            warn: None,
            fail: Some(0),
        },
        evidence,
    }
}

fn validate_subject(subject: &str) -> Vec<Evidence> {
    let Some((prefix, description)) = subject.split_once(": ") else {
        return vec![Evidence::new("subject-format", subject)];
    };

    let prefix_clean = prefix.trim_end_matches('!');
    let type_str = prefix_clean.split('(').next().unwrap_or(prefix_clean);
    let mut evidence = Vec::new();

    if !DEFAULT_TYPES
        .iter()
        .any(|t| t.eq_ignore_ascii_case(type_str))
    {
        evidence.push(Evidence::new("unknown-type", type_str));
    }

    if prefix.contains("()") {
        evidence.push(Evidence::new("empty-scope", "()"));
    }

    if description.trim().is_empty() {
        evidence.push(Evidence::new("empty-description", description));
    }

    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_without_colon_space_separator_fails() {
        let result = check_commit_message("no separator here");

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.observed, 1);
        assert_eq!(
            result.evidence,
            [Evidence::new("subject-format", "no separator here")]
        );
    }

    #[test]
    fn unknown_type_is_a_violation() {
        let result = check_commit_message("banana: do something");

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence, [Evidence::new("unknown-type", "banana")]);
    }

    #[test]
    fn empty_description_is_a_violation() {
        let result = check_commit_message("feat: ");

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence, [Evidence::new("empty-description", "")]);
    }

    #[test]
    fn whitespace_only_description_is_a_violation() {
        let result = check_commit_message("feat:   \t  ");

        assert_eq!(result.status, Status::Fail);
        assert_eq!(
            result.evidence,
            [Evidence::new("empty-description", "  \t  ")]
        );
    }

    #[test]
    fn type_matching_is_case_insensitive() {
        let result = check_commit_message("Feat: add login");

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn scope_in_parentheses_is_accepted() {
        let result = check_commit_message("feat(auth): add login");

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn empty_scope_is_a_violation() {
        let result = check_commit_message("feat(): add login");

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence, [Evidence::new("empty-scope", "()")]);
    }

    #[test]
    fn breaking_change_indicator_is_accepted() {
        let result = check_commit_message("feat!: breaking change");

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn scope_with_breaking_change_is_accepted() {
        let result = check_commit_message("feat(api)!: remove endpoint");

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn multiple_violations_produce_multiple_evidence_entries() {
        let result = check_commit_message("banana: ");

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.observed, 1);
        assert_eq!(
            result.evidence,
            [
                Evidence::new("unknown-type", "banana"),
                Evidence::new("empty-description", ""),
            ]
        );
    }

    #[test]
    fn valid_message_returns_pass_with_all_fields() {
        let result = check_commit_message("feat: add login");

        assert_eq!(result.check, "commit-message");
        assert_eq!(result.target, "feat: add login");
        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.observed, 0);
        assert_eq!(
            result.expected,
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert_eq!(result.evidence, vec![]);
    }
}
