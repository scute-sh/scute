use crate::{CheckOutcome, Evaluation, Evidence, Expected, Thresholds};

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
/// ```
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// // Valid conventional commit
/// let outcome = commit_message::check("feat(auth): add OAuth flow", &Definition::default());
/// assert!(outcome.is_pass());
/// assert!(outcome.result.unwrap().evidence.is_empty());
///
/// // Multiple violations
/// let outcome = commit_message::check("banana: ", &Definition::default());
/// assert!(outcome.is_fail());
/// assert_eq!(outcome.result.unwrap().evidence.len(), 2);
/// ```
#[must_use]
pub fn check(message: &str, definition: &Definition) -> CheckOutcome {
    let clean = strip_comments(message);
    let subject = clean.lines().next().unwrap_or("");

    CheckOutcome {
        target: subject.into(),
        result: Ok(evaluate(&clean, definition)),
    }
}

fn strip_comments(message: &str) -> String {
    message
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn evaluate(message: &str, definition: &Definition) -> Evaluation {
    let subject = message.lines().next().unwrap_or("");
    let types = definition
        .types
        .clone()
        .unwrap_or_else(|| DEFAULT_TYPES.iter().map(|&s| s.into()).collect());
    let mut evidence = validate_subject(subject, &types);
    evidence.extend(validate_structure(message));
    let observed = u64::from(!evidence.is_empty());
    let thresholds = definition.thresholds.clone().unwrap_or(DEFAULT_THRESHOLDS);

    Evaluation::new(observed, thresholds, evidence)
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
    use crate::Status;
    use googletest::prelude::*;

    #[test]
    fn message_without_colon_space_separator_fails() {
        let evaluation = evaluate("no separator here", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_eq!(evaluation.observed, 1);
        assert_that!(evaluation.evidence[0].rule, some(eq("subject-format")));
        assert_eq!(evaluation.evidence[0].found, "no separator here");
    }

    #[test]
    fn rejects_unknown_type() {
        let evaluation = evaluate("banana: do something", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("unknown-type")));
        assert_eq!(evaluation.evidence[0].found, "banana");
    }

    #[test]
    fn unknown_type_expected_lists_valid_types() {
        let evaluation = evaluate("banana: do something", &Definition::default());

        assert!(matches!(
            evaluation.evidence[0].expected,
            Some(Expected::List(_))
        ));
    }

    #[test]
    fn rejects_empty_description() {
        let evaluation = evaluate("feat: ", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("empty-description")));
    }

    #[test]
    fn rejects_whitespace_only_description() {
        let evaluation = evaluate("feat:   \t  ", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("empty-description")));
    }

    #[test]
    fn accepts_type_regardless_of_case() {
        let evaluation = evaluate("Feat: add login", &Definition::default());

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn accepts_scope_in_parentheses() {
        let evaluation = evaluate("feat(auth): add login", &Definition::default());

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn rejects_empty_scope() {
        let evaluation = evaluate("feat(): add login", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("empty-scope")));
    }

    #[test]
    fn accepts_breaking_change_indicator() {
        let evaluation = evaluate("feat!: breaking change", &Definition::default());

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn accepts_scope_with_breaking_change() {
        let evaluation = evaluate("feat(api)!: remove endpoint", &Definition::default());

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn multiple_violations_produce_multiple_evidence_entries() {
        let evaluation = evaluate("banana: ", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_eq!(evaluation.observed, 1);
        assert_eq!(evaluation.evidence.len(), 2);
        assert_that!(evaluation.evidence[0].rule, some(eq("unknown-type")));
        assert_that!(evaluation.evidence[1].rule, some(eq("empty-description")));
    }

    #[test]
    fn rejects_body_not_separated_by_blank_line() {
        let evaluation = evaluate(
            "feat: add login\nThis is not separated.",
            &Definition::default(),
        );

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("body-separator")));
        assert_eq!(evaluation.evidence[0].found, "This is not separated.");
        assert_eq!(evaluation.evidence[0].expected, None);
    }

    #[test]
    fn valid_message_with_footer_passes() {
        let evaluation = evaluate(
            "feat: add login\n\nSome body text.\n\nReviewed-by: Alice",
            &Definition::default(),
        );

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn accepts_footer_with_hash_value_format() {
        let evaluation = evaluate("fix: resolve bug\n\nFixes #123", &Definition::default());

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn rejects_malformed_footer() {
        let evaluation = evaluate(
            "feat: add login\n\nSome body.\n\nReviewed-by: Alice\nnot a valid footer",
            &Definition::default(),
        );

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("footer-format")));
        assert_eq!(evaluation.evidence[0].found, "not a valid footer");
    }

    #[test]
    fn rejects_lowercase_breaking_change_footer() {
        let evaluation = evaluate(
            "feat!: drop API\n\nbreaking change: removed endpoint",
            &Definition::default(),
        );

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(
            evaluation.evidence[0].rule,
            some(eq("breaking-change-case"))
        );
        assert_eq!(evaluation.evidence[0].found, "breaking change");
    }

    #[test]
    fn strips_git_comment_lines() {
        let outcome = check(
            "feat: add login\n# This is a git comment\n\nBody here.",
            &Definition::default(),
        );

        assert!(outcome.is_pass());
        assert!(outcome.result.unwrap().evidence.is_empty());
    }

    #[test]
    fn rejects_empty_commit_message() {
        let evaluation = evaluate("", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
        assert_that!(evaluation.evidence[0].rule, some(eq("subject-format")));
    }

    #[test]
    fn rejects_whitespace_only_commit_message() {
        let evaluation = evaluate("   \n  \n ", &Definition::default());

        assert_eq!(evaluation.status, Status::Fail);
    }

    #[test]
    fn valid_message_with_body_passes() {
        let evaluation = evaluate(
            "feat: add login\n\nThis adds the login flow.",
            &Definition::default(),
        );

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn valid_message_returns_pass_with_all_fields() {
        let evaluation = evaluate("feat: add login", &Definition::default());

        assert_eq!(evaluation.status, Status::Pass);
        assert_eq!(evaluation.observed, 0);
        assert_eq!(
            evaluation.thresholds,
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert!(evaluation.evidence.is_empty());
    }

    #[test]
    fn check_sets_target_to_subject_line() {
        let outcome = check("feat: add login", &Definition::default());

        assert_eq!(outcome.target, "feat: add login");
    }

    #[test]
    fn evaluation_thresholds_match_definition() {
        let definition = Definition {
            thresholds: Some(Thresholds {
                warn: Some(1),
                fail: Some(3),
            }),
            ..Definition::default()
        };

        let evaluation = evaluate("feat: add login", &definition);

        assert_eq!(
            evaluation.thresholds,
            Thresholds {
                warn: Some(1),
                fail: Some(3),
            }
        );
    }

    #[test]
    fn subject_format_expected_describes_format() {
        let evaluation = evaluate("no separator here", &Definition::default());

        assert_eq!(
            evaluation.evidence[0].expected,
            Some(Expected::Text("type(scope): description".into()))
        );
    }

    #[test]
    fn unknown_type_expected_reflects_config_types() {
        let definition = Definition {
            types: Some(vec!["hotfix".into(), "deploy".into()]),
            ..Definition::default()
        };

        let evaluation = evaluate("feat: add login", &definition);

        assert_eq!(
            evaluation.evidence[0].expected,
            Some(Expected::List(vec!["hotfix".into(), "deploy".into()]))
        );
    }

    #[test]
    fn footer_format_expected_describes_format() {
        let evaluation = evaluate(
            "feat: add login\n\nSome body.\n\nReviewed-by: Alice\nnot a valid footer",
            &Definition::default(),
        );

        assert_eq!(
            evaluation.evidence[0].expected,
            Some(Expected::Text("token: value | token #value".into()))
        );
    }

    #[test]
    fn breaking_change_case_expected_shows_valid_casings() {
        let evaluation = evaluate(
            "feat!: drop API\n\nbreaking change: removed endpoint",
            &Definition::default(),
        );

        assert_eq!(
            evaluation.evidence[0].expected,
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

        let evaluation = evaluate("hotfix: urgent patch", &definition);

        assert_eq!(evaluation.status, Status::Pass);
        assert!(evaluation.evidence.is_empty());
    }
}
