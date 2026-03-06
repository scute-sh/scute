//! Deterministic fitness checks for software delivery.
//!
//! Each check returns `Result<Vec<Evaluation>, ExecutionError>`.
//! Use [`report::CheckReport`] to summarize results for presentation.
//!
//! # Available checks
//!
//! - [`code_similarity::check`] — Code duplication detection
//! - [`commit_message`] — Conventional Commits validation
//! - [`dependency_freshness`] — Cargo dependency freshness

pub mod code_similarity;
pub mod commit_message;
pub mod dependency_freshness;
pub mod report;

use serde::{Deserialize, Serialize};

/// Whether a check passed, warned, or failed.
///
/// Derived by comparing the `observed` value against [`Thresholds`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
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

/// Result of evaluating a check against a single target.
///
/// ```
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// let results = commit_message::check("feat: add login", &Definition::default()).unwrap();
/// let eval = &results[0];
/// assert_eq!(eval.target, "feat: add login");
/// assert!(eval.is_pass());
/// ```
#[derive(Debug, PartialEq)]
pub struct Evaluation {
    pub target: String,
    pub outcome: Outcome,
}

impl Evaluation {
    pub fn completed(
        target: impl Into<String>,
        observed: u64,
        thresholds: Thresholds,
        evidence: Vec<Evidence>,
    ) -> Self {
        Self {
            target: target.into(),
            outcome: Outcome::completed(observed, thresholds, evidence),
        }
    }

    pub fn errored(target: impl Into<String>, error: ExecutionError) -> Self {
        Self {
            target: target.into(),
            outcome: Outcome::Errored(error),
        }
    }

    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(&self.outcome, Outcome::Completed { status, .. } if *status == Status::Pass)
    }

    #[must_use]
    pub fn is_warn(&self) -> bool {
        matches!(&self.outcome, Outcome::Completed { status, .. } if *status == Status::Warn)
    }

    #[must_use]
    pub fn is_fail(&self) -> bool {
        matches!(&self.outcome, Outcome::Completed { status, .. } if *status == Status::Fail)
    }

    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(&self.outcome, Outcome::Errored(_))
    }
}

/// What happened when a check ran against a target.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Completed {
        status: Status,
        observed: u64,
        thresholds: Thresholds,
        evidence: Vec<Evidence>,
    },
    Errored(ExecutionError),
}

impl Outcome {
    /// Create a completed outcome, deriving [`Status`] from `observed` and `thresholds`.
    #[must_use]
    pub fn completed(observed: u64, thresholds: Thresholds, evidence: Vec<Evidence>) -> Self {
        Self::Completed {
            status: derive_status(observed, &thresholds),
            observed,
            thresholds,
            evidence,
        }
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
/// use scute_core::{Evidence, Expected};
///
/// let e = Evidence::with_expected(
///     "unknown-type",
///     "banana",
///     Expected::List(vec!["feat".into(), "fix".into()]),
/// );
/// assert_eq!(e.rule.as_deref(), Some("unknown-type"));
/// assert_eq!(e.found, "banana");
/// assert!(e.expected.is_some());
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

    #[test]
    fn evaluation_is_pass_for_completed_pass() {
        let eval = Evaluation::completed(
            "test",
            0,
            Thresholds {
                warn: None,
                fail: Some(0),
            },
            vec![],
        );

        assert!(eval.is_pass());
        assert!(!eval.is_fail());
        assert!(!eval.is_warn());
        assert!(!eval.is_error());
    }

    #[test]
    fn evaluation_is_fail_for_completed_fail() {
        let eval = Evaluation::completed(
            "test",
            1,
            Thresholds {
                warn: None,
                fail: Some(0),
            },
            vec![],
        );

        assert!(eval.is_fail());
        assert!(!eval.is_pass());
    }

    #[test]
    fn evaluation_is_error_for_errored_outcome() {
        let eval = Evaluation::errored(
            "test",
            ExecutionError {
                code: "boom".into(),
                message: "broken".into(),
                recovery: "fix".into(),
            },
        );

        assert!(eval.is_error());
        assert!(!eval.is_pass());
        assert!(!eval.is_fail());
    }
}
