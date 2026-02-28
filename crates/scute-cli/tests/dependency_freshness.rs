use assert_cmd::cargo::cargo_bin_cmd;
use scute_test_utils::setup_cargo_project;

#[test]
fn passing_check_exits_with_code_0() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    );

    cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn failing_check_exits_with_non_zero() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
itoa = "=0.4.8"
"#,
    );

    cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn uses_working_directory_as_target() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    );

    let output = cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let target = json["target"].as_str().expect("target should be a string");
    assert_eq!(
        std::path::Path::new(target).canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
}

#[test]
fn recognizes_dependency_freshness_command() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized subcommand"),
        "dependency-freshness should be a recognized subcommand"
    );
}

#[test]
fn unknown_check_name_exits_with_error() {
    let output = cargo_bin_cmd!("scute")
        .args(["check", "does-not-exist"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does-not-exist"),
        "error should name the unknown check"
    );
}
