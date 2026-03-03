use rmcp::schemars;
use scute_core::{CheckOutcome, Evidence, ExecutionError, Expected, Status, Thresholds};
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

impl CheckReportSchema {
    pub fn from_outcome(check_name: &str, outcome: &CheckOutcome) -> Self {
        match &outcome.result {
            Ok(eval) => {
                let status = &eval.status;
                let is_pass = *status == Status::Pass;

                let mut passed = 0u64;
                let mut warned = 0u64;
                let mut failed = 0u64;
                let findings = if is_pass {
                    passed = 1;
                    vec![]
                } else {
                    match status {
                        Status::Warn => warned = 1,
                        Status::Fail => failed = 1,
                        Status::Pass => unreachable!(),
                    }
                    vec![FindingSchema::Completed {
                        target: outcome.target.clone(),
                        status: status.to_string(),
                        measurement: MeasurementSchema {
                            observed: eval.observed,
                            thresholds: ThresholdsSchema::from(&eval.thresholds),
                        },
                        evidence: eval.evidence.iter().map(EvidenceSchema::from).collect(),
                    }]
                };

                Self {
                    check: check_name.into(),
                    summary: Some(SummarySchema {
                        evaluated: 1,
                        passed,
                        warned,
                        failed,
                        errored: 0,
                    }),
                    findings: Some(findings),
                    error: None,
                }
            }
            Err(err) => Self {
                check: check_name.into(),
                summary: None,
                findings: None,
                error: Some(ErrorSchema::from(err)),
            },
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
