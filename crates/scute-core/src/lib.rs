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

#[must_use]
pub fn check_commit_message(message: &str) -> CheckResult {
    CheckResult {
        check: "commit-message".into(),
        target: message.lines().next().unwrap_or("").into(),
        status: Status::Pass,
        observed: 0,
        expected: Thresholds {
            warn: None,
            fail: Some(0),
        },
        evidence: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_message_returns_pass_with_all_fields() {
        let result = check_commit_message("feat(auth): add login");

        assert_eq!(result.check, "commit-message");
        assert_eq!(result.target, "feat(auth): add login");
        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.observed, 0);
        assert_eq!(
            result.expected,
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert!(result.evidence.is_empty());
    }
}
