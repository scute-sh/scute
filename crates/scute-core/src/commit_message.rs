use crate::{CheckOutcome, Evaluation, Evidence, Expected, Measurement, Thresholds, derive_status};

pub const CHECK_NAME: &str = "commit-message";

const DEFAULT_THRESHOLDS: Thresholds = Thresholds {
    warn: None,
    fail: Some(0),
};

const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

/// Configuration for a commit-message check.
///
/// Both fields are optional. When omitted, defaults apply:
/// standard Conventional Commits types and `{ fail: 0 }`.
///
/// ```
/// use scute_core::commit_message::Definition;
/// use scute_core::Thresholds;
/// use scute_core::commit_message;
///
/// let def = Definition {
///     types: Some(vec!["hotfix".into()]),
///     thresholds: Some(Thresholds { warn: None, fail: Some(0) }),
/// };
///
/// let result = commit_message::check("hotfix: urgent patch", &def);
/// assert!(result.is_pass());
///
/// let result = commit_message::check("feat: add login", &def);
/// assert!(result.is_fail());
/// ```
#[derive(Debug, Default)]
pub struct Definition {
    pub types: Option<Vec<String>>,
    pub thresholds: Option<Thresholds>,
}

/// Validate a commit message against the Conventional Commits spec.
///
/// Git comment lines (`#`-prefixed) are stripped before validation.
/// Use `Definition::default()` for standard Conventional Commits types and `{ fail: 0 }`.
///
/// # Examples
///
/// ```
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// // Valid conventional commit
/// let result = commit_message::check("feat(auth): add OAuth flow", &Definition::default());
/// assert!(result.is_pass());
/// assert!(result.evidence().is_empty());
///
/// // Multiple violations
/// let result = commit_message::check("banana: ", &Definition::default());
/// assert!(result.is_fail());
/// assert_eq!(result.evidence().len(), 2);
/// ```
#[must_use]
pub fn check(message: &str, definition: &Definition) -> CheckOutcome {
    let message: String = message
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let subject = message.lines().next().unwrap_or("");
    let types = definition
        .types
        .clone()
        .unwrap_or_else(|| DEFAULT_TYPES.iter().map(|&s| s.into()).collect());
    let mut evidence = validate_subject(subject, &types);
    evidence.extend(validate_structure(&message));
    let observed = u64::from(!evidence.is_empty());
    let thresholds = definition.thresholds.clone().unwrap_or(DEFAULT_THRESHOLDS);

    CheckOutcome {
        check: CHECK_NAME.into(),
        target: subject.into(),
        evaluation: Evaluation::new(
            derive_status(observed, &thresholds),
            Measurement {
                observed,
                thresholds,
            },
            evidence,
        ),
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

fn is_breaking_change(token: &str) -> bool {
    token.eq_ignore_ascii_case("BREAKING CHANGE") || token.eq_ignore_ascii_case("BREAKING-CHANGE")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[test]
    fn message_without_colon_space_separator_fails() {
        let result = check("no separator here", &Definition::default());

        assert!(result.is_fail());
        assert_eq!(result.observed(), 1);
        assert_that!(result.evidence()[0].rule, some(eq("subject-format")));
        assert_eq!(result.evidence()[0].found, "no separator here");
    }

    #[test]
    fn rejects_unknown_type() {
        let result = check("banana: do something", &Definition::default());

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("unknown-type")));
        assert_eq!(result.evidence()[0].found, "banana");
    }

    #[test]
    fn unknown_type_expected_lists_valid_types() {
        let result = check("banana: do something", &Definition::default());

        assert!(matches!(
            result.evidence()[0].expected,
            Some(Expected::List(_))
        ));
    }

    #[test]
    fn rejects_empty_description() {
        let result = check("feat: ", &Definition::default());

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("empty-description")));
    }

    #[test]
    fn rejects_whitespace_only_description() {
        let result = check("feat:   \t  ", &Definition::default());

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("empty-description")));
    }

    #[test]
    fn accepts_type_regardless_of_case() {
        let result = check("Feat: add login", &Definition::default());

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn accepts_scope_in_parentheses() {
        let result = check("feat(auth): add login", &Definition::default());

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn rejects_empty_scope() {
        let result = check("feat(): add login", &Definition::default());

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("empty-scope")));
    }

    #[test]
    fn accepts_breaking_change_indicator() {
        let result = check("feat!: breaking change", &Definition::default());

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn accepts_scope_with_breaking_change() {
        let result = check("feat(api)!: remove endpoint", &Definition::default());

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn multiple_violations_produce_multiple_evidence_entries() {
        let result = check("banana: ", &Definition::default());

        assert!(result.is_fail());
        assert_eq!(result.observed(), 1);
        assert_eq!(result.evidence().len(), 2);
        assert_that!(result.evidence()[0].rule, some(eq("unknown-type")));
        assert_that!(result.evidence()[1].rule, some(eq("empty-description")));
    }

    #[test]
    fn rejects_body_not_separated_by_blank_line() {
        let result = check(
            "feat: add login\nThis is not separated.",
            &Definition::default(),
        );

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("body-separator")));
        assert_eq!(result.evidence()[0].found, "This is not separated.");
        assert_eq!(result.evidence()[0].expected, None);
    }

    #[test]
    fn valid_message_with_footer_passes() {
        let result = check(
            "feat: add login\n\nSome body text.\n\nReviewed-by: Alice",
            &Definition::default(),
        );

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn accepts_footer_with_hash_value_format() {
        let result = check("fix: resolve bug\n\nFixes #123", &Definition::default());

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn rejects_malformed_footer() {
        let result = check(
            "feat: add login\n\nSome body.\n\nReviewed-by: Alice\nnot a valid footer",
            &Definition::default(),
        );

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("footer-format")));
        assert_eq!(result.evidence()[0].found, "not a valid footer");
    }

    #[test]
    fn rejects_lowercase_breaking_change_footer() {
        let result = check(
            "feat!: drop API\n\nbreaking change: removed endpoint",
            &Definition::default(),
        );

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("breaking-change-case")));
        assert_eq!(result.evidence()[0].found, "breaking change");
    }

    #[test]
    fn strips_git_comment_lines() {
        let result = check(
            "feat: add login\n# This is a git comment\n\nBody here.",
            &Definition::default(),
        );

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn rejects_empty_commit_message() {
        let result = check("", &Definition::default());

        assert!(result.is_fail());
        assert_that!(result.evidence()[0].rule, some(eq("subject-format")));
    }

    #[test]
    fn rejects_whitespace_only_commit_message() {
        let result = check("   \n  \n ", &Definition::default());

        assert!(result.is_fail());
    }

    #[test]
    fn valid_message_with_body_passes() {
        let result = check(
            "feat: add login\n\nThis adds the login flow.",
            &Definition::default(),
        );

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn valid_message_returns_pass_with_all_fields() {
        let result = check("feat: add login", &Definition::default());

        assert_eq!(result.check, "commit-message");
        assert_eq!(result.target, "feat: add login");
        assert!(result.is_pass());
        assert_eq!(result.observed(), 0);
        assert_eq!(
            *result.thresholds(),
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert!(result.evidence().is_empty());
    }

    #[test]
    fn result_thresholds_match_definition() {
        let definition = Definition {
            thresholds: Some(Thresholds {
                warn: Some(1),
                fail: Some(3),
            }),
            ..Definition::default()
        };

        let result = check("feat: add login", &definition);

        assert_eq!(
            *result.thresholds(),
            Thresholds {
                warn: Some(1),
                fail: Some(3),
            }
        );
    }

    #[test]
    fn subject_format_expected_describes_format() {
        let result = check("no separator here", &Definition::default());

        assert_eq!(
            result.evidence()[0].expected,
            Some(Expected::Text("type(scope): description".into()))
        );
    }

    #[test]
    fn unknown_type_expected_reflects_config_types() {
        let definition = Definition {
            types: Some(vec!["hotfix".into(), "deploy".into()]),
            ..Definition::default()
        };

        let result = check("feat: add login", &definition);

        assert_eq!(
            result.evidence()[0].expected,
            Some(Expected::List(vec!["hotfix".into(), "deploy".into()]))
        );
    }

    #[test]
    fn footer_format_expected_describes_format() {
        let result = check(
            "feat: add login\n\nSome body.\n\nReviewed-by: Alice\nnot a valid footer",
            &Definition::default(),
        );

        assert_eq!(
            result.evidence()[0].expected,
            Some(Expected::Text("token: value | token #value".into()))
        );
    }

    #[test]
    fn breaking_change_case_expected_shows_valid_casings() {
        let result = check(
            "feat!: drop API\n\nbreaking change: removed endpoint",
            &Definition::default(),
        );

        assert_eq!(
            result.evidence()[0].expected,
            Some(Expected::List(vec![
                "BREAKING CHANGE".into(),
                "BREAKING-CHANGE".into(),
            ]))
        );
    }

    #[test]
    fn custom_types_override_defaults() {
        let definition = Definition {
            types: Some(vec!["hotfix".into()]),
            ..Definition::default()
        };

        let result = check("hotfix: urgent patch", &definition);

        assert!(result.is_pass());
        assert!(result.evidence().is_empty());
    }
}
