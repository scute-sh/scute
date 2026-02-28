use assert_cmd::cargo::cargo_bin_cmd;
use scute_test_utils::TestProject;
use serde_json::Value;

#[test]
fn outputs_valid_json_to_stdout() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "feat: add login"])
        .output()
        .unwrap();

    let _: Value = serde_json::from_slice(&output.stdout).unwrap();
}

#[test]
fn passing_check_exits_with_code_0() {
    cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "feat: add login"])
        .assert()
        .success();
}

#[test]
fn no_argument_reads_message_from_stdin() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message"])
        .write_stdin("fix: resolve crash on startup")
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(result["target"], "fix: resolve crash on startup");
}

#[test]
fn argument_takes_precedence_over_stdin() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "feat: from argument"])
        .write_stdin("fix: from stdin")
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(result["target"], "feat: from argument");
}

#[test]
fn failing_check_exits_with_code_1() {
    cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "not a conventional commit"])
        .assert()
        .code(1);
}

#[test]
fn invalid_config_exits_with_error() {
    let dir = TestProject::new()
        .scute_config("not: valid: yaml: [")
        .build();

    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "feat: add login"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn json_output_nests_observed_and_thresholds_under_measurement() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "feat: add login"])
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(result["measurement"]["observed"].is_number());
    assert!(result["measurement"]["thresholds"].is_object());
}

#[test]
fn serializes_provided_expected_in_evidence() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "banana: do stuff"])
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(!result["evidence"][0]["expected"].is_null());
}

#[test]
fn omits_absent_expected_from_evidence() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "feat: add login\nnot separated"])
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(result["evidence"][0]["rule"], "body-separator");
    assert!(result["evidence"][0].get("expected").is_none());
}

#[test]
fn config_types_override_defaults() {
    let dir = TestProject::new()
        .scute_config(
            r"
checks:
  commit-message:
    config:
      types: [hotfix]
",
        )
        .build();

    let output = cargo_bin_cmd!("scute")
        .args(["check", "commit-message", "hotfix: urgent patch"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(result["status"], "pass");
}
