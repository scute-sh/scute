//! Deterministic fitness checks for software delivery.
//!
//! Each check produces a [`CheckOutcome`] with structured evidence.
//!
//! # Available checks
//!
//! - [`commit_message`] — Conventional Commits validation
//! - [`dependency_freshness`] — Cargo dependency freshness

pub mod commit_message;
pub mod dependency_freshness;

use serde::{Deserialize, Serialize};

/// Outcome of a threshold comparison.
///
/// Derived by comparing [`Evaluation::observed`] against [`Thresholds`].
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass => write!(f, "pass"),
            Self::Warn => write!(f, "warn"),
            Self::Fail => write!(f, "fail"),
        }
    }
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

/// Fitness assessment produced when a check executes successfully.
#[derive(Debug, PartialEq)]
pub struct Evaluation {
    pub status: Status,
    pub observed: u64,
    pub thresholds: Thresholds,
    pub evidence: Vec<Evidence>,
}

impl Evaluation {
    #[must_use]
    pub fn new(observed: u64, thresholds: Thresholds, evidence: Vec<Evidence>) -> Self {
        Self {
            status: derive_status(observed, &thresholds),
            observed,
            thresholds,
            evidence,
        }
    }
}

/// Outcome of invoking a check against a target.
///
/// The `result` carries either a successful [`Evaluation`] or an
/// [`ExecutionError`] explaining why the check couldn't run.
///
/// ```
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// let outcome = commit_message::check("feat: add login", &Definition::default());
/// assert_eq!(outcome.target, "feat: add login");
/// assert!(outcome.is_pass());
///
/// let evaluation = outcome.result.unwrap();
/// assert_eq!(evaluation.observed, 0);
/// assert!(evaluation.evidence.is_empty());
/// ```
#[derive(Debug, PartialEq)]
pub struct CheckOutcome {
    pub target: String,
    pub result: Result<Evaluation, ExecutionError>,
}

impl CheckOutcome {
    #[must_use]
    pub fn is_pass(&self) -> bool {
        self.result.as_ref().is_ok_and(|e| e.status == Status::Pass)
    }

    #[must_use]
    pub fn is_warn(&self) -> bool {
        self.result.as_ref().is_ok_and(|e| e.status == Status::Warn)
    }

    #[must_use]
    pub fn is_fail(&self) -> bool {
        self.result.as_ref().is_ok_and(|e| e.status == Status::Fail)
    }

    #[must_use]
    pub fn is_error(&self) -> bool {
        self.result.is_err()
    }
}

/// Structured error when a check cannot execute.
#[derive(Debug, PartialEq, Serialize)]
pub struct ExecutionError {
    pub code: String,
    pub message: String,
    pub recovery: String,
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
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// let outcome = commit_message::check("banana: do stuff", &Definition::default());
/// let evaluation = outcome.result.unwrap();
///
/// assert_eq!(evaluation.evidence[0].rule.as_deref(), Some("unknown-type"));
/// assert_eq!(evaluation.evidence[0].found, "banana");
/// assert!(evaluation.evidence[0].expected.is_some());
/// ```
#[derive(Debug, PartialEq, Serialize)]
pub struct Evidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub found: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Expected>,
}

impl Evidence {
    #[must_use]
    pub fn new(rule: &str, found: &str) -> Self {
        Self {
            rule: Some(rule.into()),
            location: None,
            found: found.into(),
            expected: None,
        }
    }

    #[must_use]
    pub fn with_expected(rule: &str, found: &str, expected: Expected) -> Self {
        Self {
            rule: Some(rule.into()),
            location: None,
            found: found.into(),
            expected: Some(expected),
        }
    }
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
    fn lower_is_worse_at_fail_threshold_with_warn_returns_warn() {
        let thresholds = Thresholds {
            warn: Some(70),
            fail: Some(50),
        };

        let status = derive_status(50, &thresholds);

        assert_eq!(status, Status::Warn);
    }

    #[test]
    fn lower_is_worse_at_warn_threshold_returns_pass() {
        let thresholds = Thresholds {
            warn: Some(70),
            fail: Some(50),
        };

        let status = derive_status(70, &thresholds);

        assert_eq!(status, Status::Pass);
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
    fn at_fail_threshold_with_warn_returns_warn() {
        let thresholds = Thresholds {
            warn: Some(3),
            fail: Some(10),
        };

        let status = derive_status(10, &thresholds);

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
    fn at_fail_threshold_without_warn_returns_pass() {
        let thresholds = Thresholds {
            warn: None,
            fail: Some(5),
        };

        let status = derive_status(5, &thresholds);

        assert_eq!(status, Status::Pass);
    }
}
