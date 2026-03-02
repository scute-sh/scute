use rmcp::schemars;
use scute_core::{CheckOutcome, Evidence, ExecutionError, Expected, Thresholds};
use serde::Serialize;

#[derive(Serialize, schemars::JsonSchema)]
pub struct CheckOutcomeSchema {
    /// The check that produced this outcome (e.g. `"commit-message"`).
    pub check: String,
    /// What was checked (e.g. the commit message text).
    pub target: String,
    /// Present when the check executed successfully.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<EvaluationSchema>,
    /// Present when the check could not execute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorSchema>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EvaluationSchema {
    /// The verdict: `"pass"`, `"warn"`, or `"fail"`.
    pub status: String,
    /// The observed value and the thresholds it was compared against.
    pub measurement: MeasurementSchema,
    /// Individual violations found. Absent when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceSchema>,
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

impl CheckOutcomeSchema {
    pub fn from_outcome(check_name: &str, outcome: &CheckOutcome) -> Self {
        match &outcome.result {
            Ok(evaluation) => Self {
                check: check_name.into(),
                target: outcome.target.clone(),
                evaluation: Some(EvaluationSchema {
                    status: evaluation.status.to_string(),
                    measurement: MeasurementSchema {
                        observed: evaluation.observed,
                        thresholds: ThresholdsSchema::from(&evaluation.thresholds),
                    },
                    evidence: evaluation
                        .evidence
                        .iter()
                        .map(EvidenceSchema::from)
                        .collect(),
                }),
                error: None,
            },
            Err(err) => Self {
                check: check_name.into(),
                target: outcome.target.clone(),
                evaluation: None,
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
