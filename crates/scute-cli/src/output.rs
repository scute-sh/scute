use serde::Serialize;

use scute_core::report::{CheckReport, Summary};
use scute_core::{Evidence, ExecutionError, Outcome, Status, Thresholds};

/// Observed value paired with the [`Thresholds`] used for comparison.
#[derive(Serialize)]
pub struct MeasurementJson<'a> {
    pub observed: u64,
    pub thresholds: &'a Thresholds,
}

#[derive(Serialize)]
pub struct CheckReportJson<'a> {
    pub check: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SummaryJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<FindingJson<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a ExecutionError>,
}

#[derive(Serialize)]
pub struct SummaryJson {
    pub evaluated: u64,
    pub passed: u64,
    pub warned: u64,
    pub failed: u64,
    pub errored: u64,
}

impl From<&Summary> for SummaryJson {
    fn from(s: &Summary) -> Self {
        Self {
            evaluated: s.evaluated,
            passed: s.passed,
            warned: s.warned,
            failed: s.failed,
            errored: s.errored,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum FindingJson<'a> {
    Completed {
        target: &'a str,
        status: &'a Status,
        measurement: MeasurementJson<'a>,
        #[serde(skip_serializing_if = "<[Evidence]>::is_empty")]
        evidence: &'a [Evidence],
    },
    Errored {
        target: &'a str,
        status: &'static str,
        error: &'a ExecutionError,
    },
}

impl<'a> From<&'a CheckReport> for CheckReportJson<'a> {
    fn from(report: &'a CheckReport) -> Self {
        match &report.result {
            Ok(run) => {
                let findings: Vec<FindingJson<'_>> = run
                    .non_passing_evaluations()
                    .into_iter()
                    .map(FindingJson::from)
                    .collect();

                Self {
                    check: &report.check,
                    summary: Some(SummaryJson::from(&run.summary)),
                    findings: Some(findings),
                    error: None,
                }
            }
            Err(err) => Self {
                check: &report.check,
                summary: None,
                findings: None,
                error: Some(err),
            },
        }
    }
}

impl<'a> From<&'a scute_core::Evaluation> for FindingJson<'a> {
    fn from(eval: &'a scute_core::Evaluation) -> Self {
        match &eval.outcome {
            Outcome::Completed {
                status,
                observed,
                thresholds,
                evidence,
            } => Self::Completed {
                target: &eval.target,
                status,
                measurement: MeasurementJson {
                    observed: *observed,
                    thresholds,
                },
                evidence,
            },
            Outcome::Errored(err) => Self::Errored {
                target: &eval.target,
                status: "error",
                error: err,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scute_core::{Evaluation, Expected, Outcome, Thresholds};

    fn serialize(check_name: &str, evals: Vec<Evaluation>) -> serde_json::Value {
        let report = CheckReport::new(check_name, Ok(evals));
        let json = CheckReportJson::from(&report);
        serde_json::to_value(&json).expect("serializes to JSON")
    }

    #[test]
    fn passing_check_omits_error_field() {
        let evals = vec![Evaluation {
            target: "feat: add login".into(),
            outcome: Outcome::completed(
                0,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            ),
        }];

        let json = serialize("commit-message", evals);

        assert!(json.get("error").is_none());
    }

    #[test]
    fn failing_evaluation_appears_in_findings_with_full_detail() {
        let evidence = vec![Evidence::with_expected(
            "unknown-type",
            "banana",
            Expected::List(vec!["feat".into(), "fix".into()]),
        )];
        let evals = vec![Evaluation {
            target: "banana: stuff".into(),
            outcome: Outcome::completed(
                1,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                evidence,
            ),
        }];

        let json = serialize("commit-message", evals);

        let finding = &json["findings"][0];
        assert_eq!(finding["target"], "banana: stuff");
        assert_eq!(finding["status"], "fail");
        assert_eq!(finding["measurement"]["observed"], 1);
        assert_eq!(finding["measurement"]["thresholds"]["fail"], 0);
        assert_eq!(finding["evidence"][0]["rule"], "unknown-type");
        assert_eq!(finding["evidence"][0]["found"], "banana");
        assert_eq!(
            finding["evidence"][0]["expected"],
            serde_json::json!(["feat", "fix"])
        );
    }

    #[test]
    fn finding_omits_evidence_when_empty() {
        let evals = vec![Evaluation {
            target: "src/".into(),
            outcome: Outcome::completed(
                3,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            ),
        }];

        let json = serialize("dependency-freshness", evals);

        assert!(json["findings"][0].get("evidence").is_none());
    }

    #[test]
    fn check_level_error_omits_summary_and_findings() {
        let report = CheckReport::new(
            "dependency-freshness",
            Err(ExecutionError {
                code: "invalid_target".into(),
                message: "not a Cargo project".into(),
                recovery: "point to a directory containing a Cargo.toml".into(),
            }),
        );

        let json = serde_json::to_value(CheckReportJson::from(&report)).expect("serializes");

        assert!(json.get("error").is_some());
        assert!(json.get("summary").is_none());
        assert!(json.get("findings").is_none());
    }

    #[test]
    fn finding_thresholds_omit_absent_warn() {
        let evals = vec![Evaluation {
            target: "test".into(),
            outcome: Outcome::completed(
                1,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            ),
        }];

        let json = serialize("test-check", evals);

        let thresholds = &json["findings"][0]["measurement"]["thresholds"];
        assert!(thresholds.get("warn").is_none());
        assert_eq!(thresholds["fail"], 0);
    }

    #[test]
    fn errored_evaluation_serializes_as_error_finding() {
        let evals = vec![Evaluation {
            target: "/bad/path".into(),
            outcome: Outcome::Errored(ExecutionError {
                code: "invalid_target".into(),
                message: "path does not exist".into(),
                recovery: "provide a valid path".into(),
            }),
        }];

        let json = serialize("dependency-freshness", evals);

        let finding = &json["findings"][0];
        assert_eq!(finding["status"], "error");
        assert_eq!(finding["error"]["code"], "invalid_target");
        assert!(finding.get("measurement").is_none());
    }
}
