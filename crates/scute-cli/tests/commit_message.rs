use assert_cmd::cargo::cargo_bin_cmd;
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
fn reads_message_from_stdin_when_no_argument() {
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
