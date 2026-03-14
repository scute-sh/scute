use crate::{Evaluation, ExecutionError, Outcome, Status};

/// Aggregated result of running a check.
///
/// Wraps the raw check output (`Result<Vec<Evaluation>, ExecutionError>`)
/// with summary counts and the check name.
///
/// ```
/// use scute_core::report::CheckReport;
/// use scute_core::{Evaluation, Thresholds};
///
/// let evals = vec![Evaluation::completed(
///     "feat: add login",
///     0,
///     Thresholds { warn: None, fail: Some(0) },
///     vec![],
/// )];
/// let report = CheckReport::new("commit-message", Ok(evals));
/// assert!(!report.has_failures());
/// ```
pub struct CheckReport {
    pub check: String,
    pub result: Result<CheckRun, ExecutionError>,
}

/// Successful check execution with summary and all evaluations.
#[derive(Debug)]
pub struct CheckRun {
    pub summary: Summary,
    pub evaluations: Vec<Evaluation>,
}

impl CheckRun {
    /// Evaluations that did not pass (warnings, failures, errors).
    #[must_use]
    pub fn non_passing_evaluations(&self) -> Vec<&Evaluation> {
        self.evaluations.iter().filter(|e| !e.is_pass()).collect()
    }
}

/// Counts of evaluation outcomes.
#[derive(Debug, Default)]
pub struct Summary {
    pub evaluated: u64,
    pub passed: u64,
    pub warned: u64,
    pub failed: u64,
    pub errored: u64,
}

impl Summary {
    fn tally(mut self, eval: &Evaluation) -> Self {
        self.evaluated += 1;
        match &eval.outcome {
            Outcome::Completed { status, .. } => match status {
                Status::Pass => self.passed += 1,
                Status::Warn => self.warned += 1,
                Status::Fail => self.failed += 1,
            },
            Outcome::Errored(_) => self.errored += 1,
        }
        self
    }
}

impl CheckReport {
    /// Create a report from raw check output, computing the [`Summary`].
    #[must_use]
    pub fn new(check_name: &str, result: Result<Vec<Evaluation>, ExecutionError>) -> Self {
        Self {
            check: check_name.into(),
            result: result.map(|evals| {
                let summary = summarize(&evals);
                CheckRun {
                    summary,
                    evaluations: evals,
                }
            }),
        }
    }

    /// True when any evaluation resolved to [`Status::Fail`].
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.result.as_ref().is_ok_and(|run| run.summary.failed > 0)
    }

    /// True when the check itself failed to run, or any evaluation errored.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.result.is_err()
            || self
                .result
                .as_ref()
                .is_ok_and(|run| run.summary.errored > 0)
    }
}

fn summarize(evaluations: &[Evaluation]) -> Summary {
    evaluations.iter().fold(Summary::default(), Summary::tally)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evidence, Thresholds};

    #[test]
    fn empty_evaluations_produces_zero_summary() {
        let report = CheckReport::new("test-check", Ok(vec![]));

        let run = report.result.as_ref().unwrap();
        assert_eq!(run.summary.evaluated, 0);
        assert_eq!(run.summary.passed, 0);
        assert!(!report.has_failures());
        assert!(!report.has_errors());
    }

    #[test]
    fn summary_counts_match_evaluations() {
        let evals = vec![
            passing_eval("a"),
            failing_eval("b"),
            warned_eval("c"),
            errored_eval("d"),
        ];

        let report = CheckReport::new("test-check", Ok(evals));

        let run = report.result.as_ref().unwrap();
        assert_eq!(run.summary.evaluated, 4);
        assert_eq!(run.summary.passed, 1);
        assert_eq!(run.summary.failed, 1);
        assert_eq!(run.summary.warned, 1);
        assert_eq!(run.summary.errored, 1);
    }

    #[test]
    fn check_level_error_has_no_summary() {
        let err = ExecutionError {
            code: "missing_tool".into(),
            message: "not installed".into(),
            recovery: "install it".into(),
        };

        let report = CheckReport::new("test-check", Err(err));

        assert!(report.result.is_err());
        assert_eq!(report.result.unwrap_err().code, "missing_tool");
    }

    #[test]
    fn has_failures_true_when_any_fail() {
        let evals = vec![passing_eval("a"), failing_eval("b")];

        let report = CheckReport::new("test-check", Ok(evals));

        assert!(report.has_failures());
    }

    #[test]
    fn has_failures_false_when_all_pass() {
        let report = CheckReport::new("test-check", Ok(vec![passing_eval("a")]));

        assert!(!report.has_failures());
    }

    #[test]
    fn has_errors_true_for_check_level_error() {
        let err = ExecutionError {
            code: "boom".into(),
            message: "it broke".into(),
            recovery: "fix it".into(),
        };

        let report = CheckReport::new("test-check", Err(err));

        assert!(report.has_errors());
    }

    #[test]
    fn has_errors_true_for_errored_evaluation() {
        let evals = vec![Evaluation::errored(
            "x",
            ExecutionError {
                code: "eval_err".into(),
                message: "bad".into(),
                recovery: "retry".into(),
            },
        )];

        let report = CheckReport::new("test-check", Ok(evals));

        assert!(report.has_errors());
    }

    #[test]
    fn preserves_all_evaluations_in_run() {
        let evals = vec![passing_eval("a"), failing_eval("b"), passing_eval("c")];

        let report = CheckReport::new("test-check", Ok(evals));

        let run = report.result.unwrap();
        assert_eq!(run.evaluations.len(), 3);
        assert_eq!(run.evaluations[0].target, "a");
        assert_eq!(run.evaluations[1].target, "b");
        assert_eq!(run.evaluations[2].target, "c");
    }

    #[test]
    fn non_passing_evaluations_excludes_passing() {
        let run = run_with(vec![passing_eval("a")]);

        assert!(run.non_passing_evaluations().is_empty());
    }

    #[test]
    fn non_passing_evaluations_includes_warned() {
        let run = run_with(vec![warned_eval("a")]);

        assert_eq!(run.non_passing_evaluations().len(), 1);
    }

    #[test]
    fn non_passing_evaluations_includes_failed() {
        let run = run_with(vec![failing_eval("a")]);

        assert_eq!(run.non_passing_evaluations().len(), 1);
    }

    #[test]
    fn non_passing_evaluations_includes_errored() {
        let run = run_with(vec![errored_eval("a")]);

        assert_eq!(run.non_passing_evaluations().len(), 1);
    }

    fn run_with(evals: Vec<Evaluation>) -> CheckRun {
        CheckReport::new("test-check", Ok(evals)).result.unwrap()
    }

    fn passing_eval(target: &str) -> Evaluation {
        Evaluation::completed(
            target,
            0,
            Thresholds {
                warn: None,
                fail: Some(0),
            },
            vec![],
        )
    }

    fn warned_eval(target: &str) -> Evaluation {
        Evaluation::completed(
            target,
            3,
            Thresholds {
                warn: Some(2),
                fail: Some(5),
            },
            vec![],
        )
    }

    fn failing_eval(target: &str) -> Evaluation {
        Evaluation::completed(
            target,
            1,
            Thresholds {
                warn: None,
                fail: Some(0),
            },
            vec![Evidence::new("violation", "found")],
        )
    }

    fn errored_eval(target: &str) -> Evaluation {
        Evaluation::errored(
            target,
            ExecutionError {
                code: "boom".into(),
                message: "broke".into(),
                recovery: "fix".into(),
            },
        )
    }
}
