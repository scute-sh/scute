use serde::Serialize;

use scute_core::{CheckOutcome, Evidence, ExecutionError, Status, Thresholds};

/// Serializable [`CheckOutcome`](scute_core::CheckOutcome).
///
/// Contains either [`evaluation`](Self::evaluation) (check ran successfully)
/// or [`error`](Self::error) (check couldn't execute), never both.
#[derive(Serialize)]
pub struct CheckOutcomeJson<'a> {
    /// The check that produced this outcome (e.g. `"commit-message"`).
    pub check: &'a str,
    /// What was checked (e.g. the commit message text, a directory path).
    pub target: &'a str,
    /// Present when the check executed successfully.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<EvaluationJson<'a>>,
    /// Present when the check could not execute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a ExecutionError>,
}

/// Successful check evaluation with verdict, measurement, and findings.
#[derive(Serialize)]
pub struct EvaluationJson<'a> {
    /// The verdict: `pass`, `warn`, or `fail`.
    pub status: &'a Status,
    /// The observed value and the thresholds it was compared against.
    pub measurement: MeasurementJson<'a>,
    /// Individual violations found. Omitted from JSON when empty.
    #[serde(skip_serializing_if = "<[Evidence]>::is_empty")]
    pub evidence: &'a [Evidence],
}

/// Observed value paired with the [`Thresholds`] used for comparison.
#[derive(Serialize)]
pub struct MeasurementJson<'a> {
    /// The value the check measured (count, percentage, versions behind, etc.).
    pub observed: u64,
    /// The warn/fail boundaries this measurement was compared against.
    pub thresholds: &'a Thresholds,
}

/// Convert a domain [`CheckOutcome`](scute_core::CheckOutcome) into its
/// JSON contract representation.
///
/// The resulting [`CheckOutcomeJson`] borrows from both `check_name` and
/// `outcome`, so it must be serialized before either is dropped.
#[must_use]
pub fn to_check_json<'a>(check_name: &'a str, outcome: &'a CheckOutcome) -> CheckOutcomeJson<'a> {
    match &outcome.result {
        Ok(evaluation) => CheckOutcomeJson {
            check: check_name,
            target: &outcome.target,
            evaluation: Some(EvaluationJson {
                status: &evaluation.status,
                measurement: MeasurementJson {
                    observed: evaluation.observed,
                    thresholds: &evaluation.thresholds,
                },
                evidence: &evaluation.evidence,
            }),
            error: None,
        },
        Err(error) => CheckOutcomeJson {
            check: check_name,
            target: &outcome.target,
            evaluation: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scute_core::{Evaluation, Expected};

    fn serialize(check_name: &str, outcome: &CheckOutcome) -> serde_json::Value {
        let json = to_check_json(check_name, outcome);
        serde_json::to_value(&json).expect("serializes to JSON")
    }

    #[test]
    fn passing_evaluation_nests_observed_and_thresholds_under_measurement() {
        let outcome = CheckOutcome {
            target: "feat: add login".into(),
            result: Ok(Evaluation::new(
                0,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            )),
        };

        let json = serialize("commit-message", &outcome);

        assert_eq!(json["check"], "commit-message");
        assert_eq!(json["target"], "feat: add login");
        assert_eq!(json["evaluation"]["status"], "pass");
        assert_eq!(json["evaluation"]["measurement"]["observed"], 0);
        assert_eq!(json["evaluation"]["measurement"]["thresholds"]["fail"], 0);
        assert!(json.get("error").is_none());
    }

    #[test]
    fn passing_evaluation_omits_evidence_key() {
        let outcome = CheckOutcome {
            target: "feat: add login".into(),
            result: Ok(Evaluation::new(
                0,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            )),
        };

        let json = serialize("commit-message", &outcome);

        assert!(json["evaluation"].get("evidence").is_none());
    }

    #[test]
    fn failing_evaluation_includes_evidence_with_rule_and_expected() {
        let evidence = vec![Evidence::with_expected(
            "unknown-type",
            "banana",
            Expected::List(vec!["feat".into(), "fix".into()]),
        )];
        let outcome = CheckOutcome {
            target: "banana: stuff".into(),
            result: Ok(Evaluation::new(
                1,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                evidence,
            )),
        };

        let json = serialize("commit-message", &outcome);

        assert_eq!(json["evaluation"]["status"], "fail");
        assert_eq!(json["evaluation"]["evidence"][0]["rule"], "unknown-type");
        assert_eq!(json["evaluation"]["evidence"][0]["found"], "banana");
        assert_eq!(
            json["evaluation"]["evidence"][0]["expected"],
            serde_json::json!(["feat", "fix"])
        );
    }

    #[test]
    fn evidence_without_expected_omits_expected_key() {
        let evidence = vec![Evidence::new("body-separator", "missing blank line")];
        let outcome = CheckOutcome {
            target: "feat: add login\nno blank line".into(),
            result: Ok(Evaluation::new(
                1,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                evidence,
            )),
        };

        let json = serialize("commit-message", &outcome);

        assert!(json["evaluation"]["evidence"][0].get("expected").is_none());
    }

    #[test]
    fn execution_error_includes_code_message_recovery_without_evaluation() {
        let outcome = CheckOutcome {
            target: "/nonexistent".into(),
            result: Err(ExecutionError {
                code: "invalid_target".into(),
                message: "not a Cargo project".into(),
                recovery: "point to a directory containing a Cargo.toml".into(),
            }),
        };

        let json = serialize("dependency-freshness", &outcome);

        assert_eq!(json["error"]["code"], "invalid_target");
        assert_eq!(json["error"]["message"], "not a Cargo project");
        assert_eq!(
            json["error"]["recovery"],
            "point to a directory containing a Cargo.toml"
        );
        assert!(json.get("evaluation").is_none());
    }

    #[test]
    fn thresholds_omits_absent_warn_and_fail() {
        let outcome = CheckOutcome {
            target: "test".into(),
            result: Ok(Evaluation::new(
                0,
                Thresholds {
                    warn: None,
                    fail: None,
                },
                vec![],
            )),
        };

        let json = serialize("test-check", &outcome);

        let thresholds = &json["evaluation"]["measurement"]["thresholds"];
        assert!(thresholds.get("warn").is_none());
        assert!(thresholds.get("fail").is_none());
    }

    #[test]
    fn thresholds_includes_both_warn_and_fail_when_present() {
        let outcome = CheckOutcome {
            target: "src/".into(),
            result: Ok(Evaluation::new(
                3,
                Thresholds {
                    warn: Some(5),
                    fail: Some(8),
                },
                vec![],
            )),
        };

        let json = serialize("dependency-freshness", &outcome);

        assert_eq!(json["evaluation"]["measurement"]["thresholds"]["warn"], 5);
        assert_eq!(json["evaluation"]["measurement"]["thresholds"]["fail"], 8);
    }
}
