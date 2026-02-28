//! Deterministic fitness checks for software delivery.
//!
//! Each check produces a [`CheckResult`] with structured evidence.
//!
//! # Available checks
//!
//! - [`check_commit_message`] — Conventional Commits validation
//! - [`dependency_freshness`] — Cargo dependency freshness

pub mod dependency_freshness;

use serde::{Deserialize, Serialize};

/// Outcome of a threshold comparison.
///
/// Derived by comparing [`Measurement::observed`] against [`Thresholds`].
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

/// Warn and fail boundaries for a check.
///
/// When both are set, their relative order determines direction:
/// `warn < fail` means higher is worse (e.g. violation counts),
/// `warn > fail` means lower is worse (e.g. coverage percentages).
///
/// ```
/// use scute_core::Thresholds;
///
/// // "More than 0 violations is a failure"
/// let violations = Thresholds { warn: None, fail: Some(0) };
///
/// // "Coverage below 70% warns, below 50% fails"
/// let coverage = Thresholds { warn: Some(70), fail: Some(50) };
/// ```
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Thresholds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail: Option<u64>,
}

/// Numeric measurement and thresholds for a check.
///
/// Groups the observed value with the boundaries that determine
/// [`Status`]. This keeps all trending-relevant data in one place.
///
/// ```
/// use scute_core::{Measurement, Thresholds};
///
/// let m = Measurement {
///     observed: 3,
///     thresholds: Thresholds { warn: Some(1), fail: Some(10) },
/// };
/// ```
#[derive(Debug, PartialEq, Serialize)]
pub struct Measurement {
    pub observed: u64,
    pub thresholds: Thresholds,
}

/// Result of running a check against a target.
///
/// Serializes to JSON following the
/// [check result schema](https://github.com/nomato/scute/blob/main/handbook/decisions/0001-check-result-schema.md).
///
/// ```
/// use scute_core::check_commit_message;
///
/// let result = check_commit_message("feat: add login", None);
///
/// assert_eq!(result.check, "commit-message");
/// assert_eq!(result.target, "feat: add login");
/// assert_eq!(result.measurement.observed, 0);
/// assert!(result.evidence.is_empty());
/// ```
#[derive(Debug, PartialEq, Serialize)]
pub struct CheckResult {
    pub check: String,
    pub target: String,
    pub status: Status,
    pub measurement: Measurement,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
}

impl CheckResult {
    #[must_use]
    pub fn failed(&self) -> bool {
        self.status == Status::Fail
    }
}

/// What a check expected to find instead of the violation.
///
/// Serializes without a type tag: [`Text`](Expected::Text) becomes a JSON
/// string, [`List`](Expected::List) becomes a JSON array.
///
/// ```
/// use scute_core::Expected;
///
/// let format = Expected::Text("type(scope): description".into());
/// let types = Expected::List(vec!["feat".into(), "fix".into()]);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Expected {
    Text(String),
    List(Vec<String>),
}

/// A single violation found during a check.
///
/// `rule` identifies what was violated, `found` shows what triggered it.
/// `expected` optionally carries what the check expected instead, when
/// the rule name alone isn't enough to act on.
///
/// ```
/// use scute_core::check_commit_message;
///
/// let result = check_commit_message("banana: do stuff", None);
///
/// assert_eq!(result.evidence[0].rule, "unknown-type");
/// assert_eq!(result.evidence[0].found, "banana");
/// assert!(result.evidence[0].expected.is_some());
/// ```
#[derive(Debug, PartialEq, Serialize)]
pub struct Evidence {
    pub rule: String,
    pub found: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Expected>,
}

impl Evidence {
    #[must_use]
    pub fn new(rule: &str, found: &str) -> Self {
        Self {
            rule: rule.into(),
            found: found.into(),
            expected: None,
        }
    }

    pub(crate) fn with_expected(rule: &str, found: &str, expected: Expected) -> Self {
        Self {
            rule: rule.into(),
            found: found.into(),
            expected: Some(expected),
        }
    }
}

/// Configuration for a commit-message check.
///
/// Both fields are optional. When omitted, defaults apply:
/// standard Conventional Commits types and `{ fail: 0 }`.
///
/// ```
/// use scute_core::{CommitMessageDefinition, Thresholds, check_commit_message, Status};
///
/// let def = CommitMessageDefinition {
///     types: Some(vec!["hotfix".into()]),
///     thresholds: Some(Thresholds { warn: None, fail: Some(0) }),
/// };
///
/// let result = check_commit_message("hotfix: urgent patch", Some(&def));
/// assert_eq!(result.status, Status::Pass);
///
/// let result = check_commit_message("feat: add login", Some(&def));
/// assert_eq!(result.status, Status::Fail);
/// ```
#[derive(Debug, Default)]
pub struct CommitMessageDefinition {
    pub types: Option<Vec<String>>,
    pub thresholds: Option<Thresholds>,
}

/// Check name used in [`CheckResult::check`] and config file lookup.
pub const CHECK_NAME: &str = "commit-message";

const DEFAULT_THRESHOLDS: Thresholds = Thresholds {
    warn: None,
    fail: Some(0),
};

const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

/// Validate a commit message against the Conventional Commits spec.
///
/// Pass `None` for `definition` to use defaults (standard types, `{ fail: 0 }`).
/// Git comment lines (`#`-prefixed) are stripped before validation.
///
/// # Examples
///
/// ```
/// use scute_core::{check_commit_message, Status};
///
/// // Valid conventional commit
/// let result = check_commit_message("feat(auth): add OAuth flow", None);
/// assert_eq!(result.status, Status::Pass);
/// assert!(result.evidence.is_empty());
///
/// // Multiple violations
/// let result = check_commit_message("banana: ", None);
/// assert_eq!(result.status, Status::Fail);
/// assert_eq!(result.evidence.len(), 2);
/// ```
#[must_use]
pub fn check_commit_message(
    message: &str,
    definition: Option<&CommitMessageDefinition>,
) -> CheckResult {
    let message: String = message
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let subject = message.lines().next().unwrap_or("");
    let types = definition
        .and_then(|d| d.types.clone())
        .unwrap_or_else(|| DEFAULT_TYPES.iter().map(|&s| s.into()).collect());
    let mut evidence = validate_subject(subject, &types);
    evidence.extend(validate_structure(&message));
    let observed = u64::from(!evidence.is_empty());
    let thresholds = definition
        .and_then(|d| d.thresholds.clone())
        .unwrap_or(DEFAULT_THRESHOLDS);

    CheckResult {
        check: CHECK_NAME.into(),
        target: subject.into(),
        status: derive_status(observed, &thresholds),
        measurement: Measurement {
            observed,
            thresholds,
        },
        evidence,
    }
}

fn validate_subject(subject: &str, types: &[String]) -> Vec<Evidence> {
    let Some((prefix, description)) = subject.split_once(": ") else {
        return vec![Evidence::with_expected(
            "subject-format",
            subject,
            Expected::Text("type(scope): description".into()),
        )];
    };

    let prefix_clean = prefix.trim_end_matches('!');
    let type_str = prefix_clean.split('(').next().unwrap_or(prefix_clean);
    let mut evidence = Vec::new();

    let type_known = types.iter().any(|t| t.eq_ignore_ascii_case(type_str));
    if !type_known {
        evidence.push(Evidence::with_expected(
            "unknown-type",
            type_str,
            Expected::List(types.to_vec()),
        ));
    }

    if prefix.contains("()") {
        evidence.push(Evidence::new("empty-scope", "()"));
    }

    if description.trim().is_empty() {
        evidence.push(Evidence::new("empty-description", description));
    }

    evidence
}

fn validate_structure(message: &str) -> Vec<Evidence> {
    let mut lines = message.lines();
    let _subject = lines.next();
    let second_line = lines.next();

    if let Some(line) = second_line
        && !line.is_empty()
    {
        return vec![Evidence::new("body-separator", line)];
    }

    let paragraphs: Vec<&str> = message.split("\n\n").collect();
    if paragraphs.len() >= 2 {
        return validate_footers(paragraphs.last().unwrap());
    }

    vec![]
}

fn validate_footers(paragraph: &str) -> Vec<Evidence> {
    let lines: Vec<&str> = paragraph.lines().collect();
    if !lines.iter().any(|l| is_footer_line(l)) {
        return vec![];
    }

    let mut evidence = Vec::new();
    for line in &lines {
        match footer_token(line) {
            Some(token)
                if is_breaking_change(token)
                    && token != "BREAKING CHANGE"
                    && token != "BREAKING-CHANGE" =>
            {
                evidence.push(Evidence::with_expected(
                    "breaking-change-case",
                    token,
                    Expected::List(vec!["BREAKING CHANGE".into(), "BREAKING-CHANGE".into()]),
                ));
            }
            None => {
                evidence.push(Evidence::with_expected(
                    "footer-format",
                    line,
                    Expected::Text("token: value | token #value".into()),
                ));
            }
            _ => {}
        }
    }
    evidence
}

fn is_footer_line(line: &str) -> bool {
    footer_token(line).is_some()
}

fn footer_token(line: &str) -> Option<&str> {
    if let Some((token, _)) = line.split_once(": ")
        && is_footer_token(token)
    {
        return Some(token);
    }
    if let Some((token, _)) = line.split_once(" #")
        && is_footer_token(token)
    {
        return Some(token);
    }
    None
}

fn is_footer_token(token: &str) -> bool {
    is_breaking_change(token)
        || (!token.is_empty() && token.chars().all(|c| c.is_alphanumeric() || c == '-'))
}

pub(crate) fn derive_status(observed: u64, thresholds: &Thresholds) -> Status {
    let higher_is_worse = match (thresholds.warn, thresholds.fail) {
        (Some(w), Some(f)) => w < f,
        _ => true,
    };

    let exceeds = if higher_is_worse {
        |observed: u64, threshold: u64| observed > threshold
    } else {
        |observed: u64, threshold: u64| observed < threshold
    };

    if let Some(fail) = thresholds.fail
        && exceeds(observed, fail)
    {
        return Status::Fail;
    }

    if let Some(warn) = thresholds.warn
        && exceeds(observed, warn)
    {
        return Status::Warn;
    }

    Status::Pass
}

fn is_breaking_change(token: &str) -> bool {
    token.eq_ignore_ascii_case("BREAKING CHANGE") || token.eq_ignore_ascii_case("BREAKING-CHANGE")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_is_worse_below_fail_threshold_returns_fail() {
        let thresholds = Thresholds {
            warn: Some(70),
            fail: Some(50),
        };

        let status = derive_status(40, &thresholds);

        assert_eq!(status, Status::Fail);
    }

    #[test]
    fn lower_is_worse_between_thresholds_returns_warn() {
        let thresholds = Thresholds {
            warn: Some(70),
            fail: Some(50),
        };

        let status = derive_status(60, &thresholds);

        assert_eq!(status, Status::Warn);
    }

    #[test]
    fn lower_is_worse_above_warn_threshold_returns_pass() {
        let thresholds = Thresholds {
            warn: Some(70),
            fail: Some(50),
        };

        let status = derive_status(80, &thresholds);

        assert_eq!(status, Status::Pass);
    }

    #[test]
    fn below_fail_with_no_warn_returns_pass() {
        let thresholds = Thresholds {
            warn: None,
            fail: Some(5),
        };

        let status = derive_status(3, &thresholds);

        assert_eq!(status, Status::Pass);
    }

    #[test]
    fn at_warn_threshold_returns_pass() {
        let thresholds = Thresholds {
            warn: Some(5),
            fail: Some(10),
        };

        let status = derive_status(5, &thresholds);

        assert_eq!(status, Status::Pass);
    }

    #[test]
    fn between_warn_and_fail_returns_warn() {
        let thresholds = Thresholds {
            warn: Some(3),
            fail: Some(10),
        };

        let status = derive_status(5, &thresholds);

        assert_eq!(status, Status::Warn);
    }

    #[test]
    fn above_fail_threshold_returns_fail() {
        let thresholds = Thresholds {
            warn: None,
            fail: Some(5),
        };

        let status = derive_status(10, &thresholds);

        assert_eq!(status, Status::Fail);
    }

    #[test]
    fn message_without_colon_space_separator_fails() {
        let result = check_commit_message("no separator here", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.measurement.observed, 1);
        assert_eq!(result.evidence[0].rule, "subject-format");
        assert_eq!(result.evidence[0].found, "no separator here");
    }

    #[test]
    fn rejects_unknown_type() {
        let result = check_commit_message("banana: do something", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "unknown-type");
        assert_eq!(result.evidence[0].found, "banana");
    }

    #[test]
    fn unknown_type_expected_lists_valid_types() {
        let result = check_commit_message("banana: do something", None);

        assert!(matches!(
            result.evidence[0].expected,
            Some(Expected::List(_))
        ));
    }

    #[test]
    fn rejects_empty_description() {
        let result = check_commit_message("feat: ", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "empty-description");
    }

    #[test]
    fn rejects_whitespace_only_description() {
        let result = check_commit_message("feat:   \t  ", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "empty-description");
    }

    #[test]
    fn accepts_type_regardless_of_case() {
        let result = check_commit_message("Feat: add login", None);

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn accepts_scope_in_parentheses() {
        let result = check_commit_message("feat(auth): add login", None);

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn rejects_empty_scope() {
        let result = check_commit_message("feat(): add login", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "empty-scope");
    }

    #[test]
    fn accepts_breaking_change_indicator() {
        let result = check_commit_message("feat!: breaking change", None);

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn accepts_scope_with_breaking_change() {
        let result = check_commit_message("feat(api)!: remove endpoint", None);

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn multiple_violations_produce_multiple_evidence_entries() {
        let result = check_commit_message("banana: ", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.measurement.observed, 1);
        assert_eq!(result.evidence.len(), 2);
        assert_eq!(result.evidence[0].rule, "unknown-type");
        assert_eq!(result.evidence[1].rule, "empty-description");
    }

    #[test]
    fn rejects_body_not_separated_by_blank_line() {
        let result = check_commit_message("feat: add login\nThis is not separated.", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "body-separator");
        assert_eq!(result.evidence[0].found, "This is not separated.");
        assert_eq!(result.evidence[0].expected, None);
    }

    #[test]
    fn valid_message_with_footer_passes() {
        let result = check_commit_message(
            "feat: add login\n\nSome body text.\n\nReviewed-by: Alice",
            None,
        );

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn accepts_footer_with_hash_value_format() {
        let result = check_commit_message("fix: resolve bug\n\nFixes #123", None);

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn rejects_malformed_footer() {
        let result = check_commit_message(
            "feat: add login\n\nSome body.\n\nReviewed-by: Alice\nnot a valid footer",
            None,
        );

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "footer-format");
        assert_eq!(result.evidence[0].found, "not a valid footer");
    }

    #[test]
    fn rejects_lowercase_breaking_change_footer() {
        let result =
            check_commit_message("feat!: drop API\n\nbreaking change: removed endpoint", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "breaking-change-case");
        assert_eq!(result.evidence[0].found, "breaking change");
    }

    #[test]
    fn strips_git_comment_lines() {
        let result = check_commit_message(
            "feat: add login\n# This is a git comment\n\nBody here.",
            None,
        );

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn rejects_empty_commit_message() {
        let result = check_commit_message("", None);

        assert_eq!(result.status, Status::Fail);
        assert_eq!(result.evidence[0].rule, "subject-format");
    }

    #[test]
    fn rejects_whitespace_only_commit_message() {
        let result = check_commit_message("   \n  \n ", None);

        assert_eq!(result.status, Status::Fail);
    }

    #[test]
    fn valid_message_with_body_passes() {
        let result = check_commit_message("feat: add login\n\nThis adds the login flow.", None);

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn valid_message_returns_pass_with_all_fields() {
        let result = check_commit_message("feat: add login", None);

        assert_eq!(result.check, "commit-message");
        assert_eq!(result.target, "feat: add login");
        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.measurement.observed, 0);
        assert_eq!(
            result.measurement.thresholds,
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert_eq!(result.evidence, vec![]);
    }

    #[test]
    fn result_thresholds_match_definition() {
        let definition = CommitMessageDefinition {
            thresholds: Some(Thresholds {
                warn: Some(1),
                fail: Some(3),
            }),
            ..CommitMessageDefinition::default()
        };

        let result = check_commit_message("feat: add login", Some(&definition));

        assert_eq!(
            result.measurement.thresholds,
            Thresholds {
                warn: Some(1),
                fail: Some(3),
            }
        );
    }

    #[test]
    fn subject_format_expected_describes_format() {
        let result = check_commit_message("no separator here", None);

        assert_eq!(
            result.evidence[0].expected,
            Some(Expected::Text("type(scope): description".into()))
        );
    }

    #[test]
    fn unknown_type_expected_reflects_config_types() {
        let definition = CommitMessageDefinition {
            types: Some(vec!["hotfix".into(), "deploy".into()]),
            ..CommitMessageDefinition::default()
        };

        let result = check_commit_message("feat: add login", Some(&definition));

        assert_eq!(
            result.evidence[0].expected,
            Some(Expected::List(vec!["hotfix".into(), "deploy".into()]))
        );
    }

    #[test]
    fn footer_format_expected_describes_format() {
        let result = check_commit_message(
            "feat: add login\n\nSome body.\n\nReviewed-by: Alice\nnot a valid footer",
            None,
        );

        assert_eq!(
            result.evidence[0].expected,
            Some(Expected::Text("token: value | token #value".into()))
        );
    }

    #[test]
    fn breaking_change_case_expected_shows_valid_casings() {
        let result =
            check_commit_message("feat!: drop API\n\nbreaking change: removed endpoint", None);

        assert_eq!(
            result.evidence[0].expected,
            Some(Expected::List(vec![
                "BREAKING CHANGE".into(),
                "BREAKING-CHANGE".into(),
            ]))
        );
    }

    #[test]
    fn custom_types_override_defaults() {
        let definition = CommitMessageDefinition {
            types: Some(vec!["hotfix".into()]),
            ..CommitMessageDefinition::default()
        };

        let result = check_commit_message("hotfix: urgent patch", Some(&definition));

        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.evidence, vec![]);
    }
}
