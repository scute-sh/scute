use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

fn setup_cargo_project(cargo_toml: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), cargo_toml).unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir(&src).unwrap();
    std::fs::write(src.join("lib.rs"), "").unwrap();
    std::process::Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    dir
}

#[test]
fn pass_exits_with_code_0() {
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
fn fail_exits_with_non_zero() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "=0.7.3"
"#,
    );

    cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn target_is_the_working_directory() {
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
fn dependency_freshness_is_a_recognized_command() {
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
