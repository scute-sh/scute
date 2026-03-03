use serde::Serialize;

use scute_core::{CheckOutcome, Evidence, ExecutionError, Status, Thresholds};

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

/// Build a check report from a single [`CheckOutcome`].
///
/// When the outcome is `Ok`, produces summary + findings.
/// When the outcome is `Err`, produces a check-level error.
#[must_use]
pub fn to_report_json<'a>(check_name: &'a str, outcome: &'a CheckOutcome) -> CheckReportJson<'a> {
    match &outcome.result {
        Ok(_) => {
            let (summary, findings) = summarize_outcomes(std::slice::from_ref(outcome));
            CheckReportJson {
                check: check_name,
                summary: Some(summary),
                findings: Some(findings),
                error: None,
            }
        }
        Err(error) => CheckReportJson {
            check: check_name,
            summary: None,
            findings: None,
            error: Some(error),
        },
    }
}

fn summarize_outcomes(outcomes: &[CheckOutcome]) -> (SummaryJson, Vec<FindingJson<'_>>) {
    let mut passed = 0u64;
    let mut warned = 0u64;
    let mut failed = 0u64;
    let mut errored = 0u64;
    let mut findings = Vec::new();

    for outcome in outcomes {
        match &outcome.result {
            Ok(eval) => {
                match eval.status {
                    Status::Pass => passed += 1,
                    Status::Warn => warned += 1,
                    Status::Fail => failed += 1,
                }
                if eval.status != Status::Pass {
                    findings.push(FindingJson::Completed {
                        target: &outcome.target,
                        status: &eval.status,
                        measurement: MeasurementJson {
                            observed: eval.observed,
                            thresholds: &eval.thresholds,
                        },
                        evidence: &eval.evidence,
                    });
                }
            }
            Err(err) => {
                errored += 1;
                findings.push(FindingJson::Errored {
                    target: &outcome.target,
                    status: "error",
                    error: err,
                });
            }
        }
    }

    let evaluated = passed + warned + failed + errored;
    (
        SummaryJson {
            evaluated,
            passed,
            warned,
            failed,
            errored,
        },
        findings,
    )
}

#[cfg(test)]
fn to_check_report_json<'a>(
    check_name: &'a str,
    result: &'a Result<&[CheckOutcome], ExecutionError>,
) -> CheckReportJson<'a> {
    match result {
        Ok(outcomes) => {
            let (summary, findings) = summarize_outcomes(outcomes);
            CheckReportJson {
                check: check_name,
                summary: Some(summary),
                findings: Some(findings),
                error: None,
            }
        }
        Err(error) => CheckReportJson {
            check: check_name,
            summary: None,
            findings: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scute_core::{Evaluation, Expected};

    fn serialize(check_name: &str, outcomes: &[CheckOutcome]) -> serde_json::Value {
        let result: Result<&[CheckOutcome], ExecutionError> = Ok(outcomes);
        let json = to_check_report_json(check_name, &result);
        serde_json::to_value(&json).expect("serializes to JSON")
    }

    #[test]
    fn passing_check_produces_summary_and_empty_findings() {
        let outcomes = vec![CheckOutcome {
            target: "feat: add login".into(),
            result: Ok(Evaluation::new(
                0,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            )),
        }];

        let json = serialize("commit-message", &outcomes);

        assert_eq!(json["check"], "commit-message");
        assert_eq!(json["summary"]["evaluated"], 1);
        assert_eq!(json["summary"]["passed"], 1);
        assert_eq!(json["summary"]["warned"], 0);
        assert_eq!(json["summary"]["failed"], 0);
        assert_eq!(json["summary"]["errored"], 0);
        assert_eq!(json["findings"], serde_json::json!([]));
        assert!(json.get("error").is_none());
    }

    #[test]
    fn failing_evaluation_appears_in_findings_with_full_detail() {
        let evidence = vec![Evidence::with_expected(
            "unknown-type",
            "banana",
            Expected::List(vec!["feat".into(), "fix".into()]),
        )];
        let outcomes = vec![CheckOutcome {
            target: "banana: stuff".into(),
            result: Ok(Evaluation::new(
                1,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                evidence,
            )),
        }];

        let json = serialize("commit-message", &outcomes);

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
    fn passing_evaluations_excluded_from_findings() {
        let outcomes = vec![
            CheckOutcome {
                target: "feat: good".into(),
                result: Ok(Evaluation::new(
                    0,
                    Thresholds {
                        warn: None,
                        fail: Some(0),
                    },
                    vec![],
                )),
            },
            CheckOutcome {
                target: "banana: bad".into(),
                result: Ok(Evaluation::new(
                    1,
                    Thresholds {
                        warn: None,
                        fail: Some(0),
                    },
                    vec![Evidence::new("unknown-type", "banana")],
                )),
            },
        ];

        let json = serialize("commit-message", &outcomes);

        assert_eq!(json["summary"]["evaluated"], 2);
        assert_eq!(json["summary"]["passed"], 1);
        assert_eq!(json["summary"]["failed"], 1);
        assert_eq!(json["findings"].as_array().unwrap().len(), 1);
        assert_eq!(json["findings"][0]["target"], "banana: bad");
    }

    #[test]
    fn finding_omits_evidence_when_empty() {
        let outcomes = vec![CheckOutcome {
            target: "src/".into(),
            result: Ok(Evaluation::new(
                3,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            )),
        }];

        let json = serialize("dependency-freshness", &outcomes);

        assert!(json["findings"][0].get("evidence").is_none());
    }

    #[test]
    fn check_level_error_produces_error_without_summary() {
        let result: Result<&[CheckOutcome], ExecutionError> = Err(ExecutionError {
            code: "invalid_target".into(),
            message: "not a Cargo project".into(),
            recovery: "point to a directory containing a Cargo.toml".into(),
        });

        let json = {
            let report = to_check_report_json("dependency-freshness", &result);
            serde_json::to_value(&report).expect("serializes")
        };

        assert_eq!(json["check"], "dependency-freshness");
        assert_eq!(json["error"]["code"], "invalid_target");
        assert_eq!(json["error"]["message"], "not a Cargo project");
        assert!(json.get("summary").is_none());
        assert!(json.get("findings").is_none());
    }

    #[test]
    fn finding_thresholds_omit_absent_warn() {
        let outcomes = vec![CheckOutcome {
            target: "test".into(),
            result: Ok(Evaluation::new(
                1,
                Thresholds {
                    warn: None,
                    fail: Some(0),
                },
                vec![],
            )),
        }];

        let json = serialize("test-check", &outcomes);

        let thresholds = &json["findings"][0]["measurement"]["thresholds"];
        assert!(thresholds.get("warn").is_none());
        assert_eq!(thresholds["fail"], 0);
    }

    #[test]
    fn errored_evaluation_appears_in_findings_with_error_object() {
        let outcomes = vec![CheckOutcome {
            target: "/bad/path".into(),
            result: Err(ExecutionError {
                code: "invalid_target".into(),
                message: "path does not exist".into(),
                recovery: "provide a valid path".into(),
            }),
        }];

        let json = serialize("dependency-freshness", &outcomes);

        assert_eq!(json["summary"]["evaluated"], 1);
        assert_eq!(json["summary"]["errored"], 1);
        let finding = &json["findings"][0];
        assert_eq!(finding["target"], "/bad/path");
        assert_eq!(finding["status"], "error");
        assert_eq!(finding["error"]["code"], "invalid_target");
        assert!(finding.get("measurement").is_none());
    }
}
