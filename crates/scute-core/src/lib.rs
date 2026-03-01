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

/// Fitness assessment produced when a check executes successfully.
#[derive(Debug, PartialEq, Serialize)]
pub struct Evaluation {
    pub status: Status,
    pub measurement: Measurement,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
}

impl Evaluation {
    #[must_use]
    pub fn new(status: Status, measurement: Measurement, evidence: Vec<Evidence>) -> Self {
        Self {
            status,
            measurement,
            evidence,
        }
    }
}

/// Outcome of invoking a check against a target.
///
/// ```
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// let result = commit_message::check("feat: add login", &Definition::default());
///
/// assert_eq!(result.check, "commit-message");
/// assert_eq!(result.target, "feat: add login");
/// assert_eq!(result.observed(), 0);
/// assert!(result.evidence().is_empty());
/// ```
#[derive(Debug, PartialEq, Serialize)]
pub struct CheckOutcome {
    pub check: String,
    pub target: String,
    pub evaluation: Evaluation,
}

impl CheckOutcome {
    #[must_use]
    pub fn is_pass(&self) -> bool {
        self.evaluation.status == Status::Pass
    }

    #[must_use]
    pub fn is_warn(&self) -> bool {
        self.evaluation.status == Status::Warn
    }

    #[must_use]
    pub fn is_fail(&self) -> bool {
        self.evaluation.status == Status::Fail
    }

    #[must_use]
    pub fn observed(&self) -> u64 {
        self.evaluation.measurement.observed
    }

    #[must_use]
    pub fn evidence(&self) -> &[Evidence] {
        &self.evaluation.evidence
    }

    #[must_use]
    pub fn thresholds(&self) -> &Thresholds {
        &self.evaluation.measurement.thresholds
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
/// use scute_core::commit_message;
/// use scute_core::commit_message::Definition;
///
/// let result = commit_message::check("banana: do stuff", &Definition::default());
///
/// assert_eq!(result.evidence()[0].rule.as_deref(), Some("unknown-type"));
/// assert_eq!(result.evidence()[0].found, "banana");
/// assert!(result.evidence()[0].expected.is_some());
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

    pub(crate) fn with_expected(rule: &str, found: &str, expected: Expected) -> Self {
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
}
