use tempfile::TempDir;

use crate::{Backend, CheckResult, ListChecksResult, target_bin};

pub(crate) struct CliBackend {
    pub(crate) stdin: bool,
}

impl Backend for CliBackend {
    fn check(&self, dir: TempDir, args: &[&str]) -> Box<dyn CheckResult> {
        Box::new(CliCheckResult::run(dir, args, self.stdin))
    }

    fn list_checks(&self, dir: TempDir) -> Box<dyn ListChecksResult> {
        let output = assert_cmd::Command::new(target_bin("scute"))
            .args(["check", "list"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let checks: Vec<String> = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("expected JSON array from `scute check list`: {e}\nstdout: {stdout}")
        });
        Box::new(CliListChecksResult { _dir: dir, checks })
    }
}

struct CliCheckResult {
    dir: TempDir,
    exit_code: i32,
    json: Option<serde_json::Value>,
    stderr: String,
}

impl CliCheckResult {
    fn run(dir: TempDir, args: &[&str], stdin: bool) -> Self {
        let mut cmd = assert_cmd::Command::new(target_bin("scute"));
        cmd.current_dir(dir.path());
        if stdin {
            let message = args.last().expect("CliStdin requires message in args");
            cmd.args(&args[..args.len() - 1])
                .write_stdin(message.to_string());
        } else {
            cmd.args(args);
        }
        let output = cmd.output().unwrap();
        Self {
            dir,
            exit_code: output.status.code().unwrap_or(-1),
            json: serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).ok(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn json(&self) -> &serde_json::Value {
        self.json
            .as_ref()
            .expect("expected valid JSON in stdout, got none")
    }
}

impl CliCheckResult {
    fn findings(&self) -> &Vec<serde_json::Value> {
        self.json()["findings"]
            .as_array()
            .expect("findings should be an array")
    }

    fn first_finding(&self) -> &serde_json::Value {
        self.findings()
            .first()
            .expect("expected at least one finding")
    }
}

impl CheckResult for CliCheckResult {
    fn expect_pass(&self) -> &dyn CheckResult {
        let summary = &self.json()["summary"];
        assert!(
            summary["failed"] == 0
                && summary["errored"] == 0
                && summary["passed"].as_u64() > Some(0),
            "expected pass, got: {}",
            self.json()
        );
        assert_eq!(
            self.exit_code, 0,
            "expected exit 0, stderr: {}",
            self.stderr
        );
        self
    }

    fn expect_warn(&self) -> &dyn CheckResult {
        let summary = &self.json()["summary"];
        assert!(
            summary["warned"].as_u64() > Some(0),
            "expected at least one warn, got: {}",
            self.json()
        );
        assert_eq!(
            self.exit_code, 0,
            "expected exit 0 for warn, stderr: {}",
            self.stderr
        );
        self
    }

    fn expect_fail(&self) -> &dyn CheckResult {
        let summary = &self.json()["summary"];
        assert!(
            summary["failed"].as_u64() > Some(0),
            "expected at least one fail, got: {}",
            self.json()
        );
        assert_eq!(self.exit_code, 1, "expected exit 1 for fail");
        self
    }

    fn expect_target(&self, expected: &str) -> &dyn CheckResult {
        assert_eq!(self.first_finding()["target"], expected);
        self
    }

    fn expect_target_contains(&self, substring: &str) -> &dyn CheckResult {
        let target = self.first_finding()["target"]
            .as_str()
            .expect("target should be a string");
        assert!(
            target.contains(substring),
            "expected target to contain '{substring}', got '{target}'"
        );
        self
    }

    fn expect_target_matches_dir(&self) -> &dyn CheckResult {
        let target = self.first_finding()["target"]
            .as_str()
            .expect("target should be a string");
        assert_eq!(
            std::path::Path::new(target).canonicalize().unwrap(),
            self.dir.path().canonicalize().unwrap()
        );
        self
    }

    fn expect_observed(&self, expected: u64) -> &dyn CheckResult {
        assert_eq!(self.first_finding()["measurement"]["observed"], expected);
        self
    }

    fn expect_evidence_rule(&self, index: usize, rule: &str) -> &dyn CheckResult {
        assert_eq!(self.first_finding()["evidence"][index]["rule"], rule);
        self
    }

    fn expect_evidence_has_expected(&self, index: usize) -> &dyn CheckResult {
        assert!(
            !self.first_finding()["evidence"][index]["expected"].is_null(),
            "expected evidence[{index}].expected to be present"
        );
        self
    }

    fn expect_evidence_no_expected(&self, index: usize) -> &dyn CheckResult {
        assert!(
            self.first_finding()["evidence"][index]
                .get("expected")
                .is_none(),
            "expected evidence[{index}].expected to be absent"
        );
        self
    }

    fn expect_finding_count(&self, expected: usize) -> &dyn CheckResult {
        assert_eq!(
            self.findings().len(),
            expected,
            "expected {expected} findings, got {}",
            self.findings().len()
        );
        self
    }

    fn expect_no_findings(&self) -> &dyn CheckResult {
        assert!(
            self.findings().is_empty(),
            "expected no findings, got: {:?}",
            self.findings()
        );
        self
    }

    fn expect_error(&self, code: &str) -> &dyn CheckResult {
        let error = &self.json()["error"];
        assert_eq!(error["code"], code, "got: {}", self.json());
        assert!(
            error["message"].is_string(),
            "error.message should be present"
        );
        assert!(
            error["recovery"].is_string(),
            "error.recovery should be present"
        );
        assert_eq!(self.exit_code, 2, "expected exit 2 for error");
        self
    }

    fn debug(&self) -> &dyn CheckResult {
        eprintln!("exit_code: {}", self.exit_code);
        eprintln!(
            "stdout: {}",
            self.json
                .as_ref()
                .map_or("(none)".into(), std::string::ToString::to_string)
        );
        eprintln!("stderr: {}", self.stderr);
        self
    }
}

struct CliListChecksResult {
    _dir: TempDir,
    checks: Vec<String>,
}

impl ListChecksResult for CliListChecksResult {
    fn expect_contains(&self, name: &str) -> &dyn ListChecksResult {
        assert!(
            self.checks.iter().any(|c| c == name),
            "expected check '{name}' in {:?}",
            self.checks
        );
        self
    }
}
