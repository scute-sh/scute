use rmcp::schemars;
use scute_core::report::{CheckReport, Summary};
use scute_core::{Evidence, ExecutionError, Expected, Outcome, Thresholds};
use serde::Serialize;

#[derive(Serialize, schemars::JsonSchema)]
pub struct CheckReportSchema {
    /// The check that produced this report (e.g. `"commit-message"`).
    pub check: String,
    /// Counts of evaluated, passed, warned, failed, and errored evaluations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SummarySchema>,
    /// Non-passing evaluations. Empty array when all pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<FindingSchema>>,
    /// Present when the check could not execute at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorSchema>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SummarySchema {
    pub evaluated: u64,
    pub passed: u64,
    pub warned: u64,
    pub failed: u64,
    pub errored: u64,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(untagged)]
pub enum FindingSchema {
    Completed {
        target: String,
        status: String,
        measurement: MeasurementSchema,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        evidence: Vec<EvidenceSchema>,
    },
    Errored {
        target: String,
        status: String,
        error: ErrorSchema,
    },
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MeasurementSchema {
    /// The value the check measured.
    pub observed: u64,
    /// The warn/fail boundaries this measurement was compared against.
    pub thresholds: ThresholdsSchema,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ThresholdsSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail: Option<u64>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EvidenceSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub found: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<ExpectedSchema>,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(untagged)]
pub enum ExpectedSchema {
    Text(String),
    List(Vec<String>),
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ErrorSchema {
    pub code: String,
    pub message: String,
    pub recovery: String,
}

impl From<&CheckReport> for CheckReportSchema {
    fn from(report: &CheckReport) -> Self {
        match &report.result {
            Ok(run) => {
                let findings: Vec<FindingSchema> = run
                    .non_passing_evaluations()
                    .into_iter()
                    .map(FindingSchema::from)
                    .collect();

                Self {
                    check: report.check.clone(),
                    summary: Some(SummarySchema::from(&run.summary)),
                    findings: Some(findings),
                    error: None,
                }
            }
            Err(err) => Self {
                check: report.check.clone(),
                summary: None,
                findings: None,
                error: Some(ErrorSchema::from(err)),
            },
        }
    }
}

impl From<&scute_core::Evaluation> for FindingSchema {
    fn from(eval: &scute_core::Evaluation) -> Self {
        match &eval.outcome {
            Outcome::Completed {
                status,
                observed,
                thresholds,
                evidence,
            } => Self::Completed {
                target: eval.target.clone(),
                status: status.to_string(),
                measurement: MeasurementSchema {
                    observed: *observed,
                    thresholds: ThresholdsSchema::from(thresholds),
                },
                evidence: evidence.iter().map(EvidenceSchema::from).collect(),
            },
            Outcome::Errored(err) => Self::Errored {
                target: eval.target.clone(),
                status: "error".into(),
                error: ErrorSchema::from(err),
            },
        }
    }
}

impl From<&Summary> for SummarySchema {
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

impl From<&Thresholds> for ThresholdsSchema {
    fn from(t: &Thresholds) -> Self {
        Self {
            warn: t.warn,
            fail: t.fail,
        }
    }
}

impl From<&Evidence> for EvidenceSchema {
    fn from(e: &Evidence) -> Self {
        Self {
            rule: e.rule.clone(),
            location: e.location.clone(),
            found: e.found.clone(),
            expected: e.expected.as_ref().map(ExpectedSchema::from),
        }
    }
}

impl From<&Expected> for ExpectedSchema {
    fn from(e: &Expected) -> Self {
        match e {
            Expected::Text(s) => Self::Text(s.clone()),
            Expected::List(v) => Self::List(v.clone()),
        }
    }
}

impl From<&ExecutionError> for ErrorSchema {
    fn from(e: &ExecutionError) -> Self {
        Self {
            code: e.code.clone(),
            message: e.message.clone(),
            recovery: e.recovery.clone(),
        }
    }
}
