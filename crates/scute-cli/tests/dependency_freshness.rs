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
fn evidence_contains_outdated_dep_name_and_versions() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "=0.7.3"
"#,
    );

    let output = cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let evidence = json["evidence"]
        .as_array()
        .expect("evidence should be an array");
    let rand_entry = evidence
        .iter()
        .find(|e| e["found"].as_str().unwrap_or("").starts_with("rand "));
    assert!(
        rand_entry.is_some(),
        "evidence should contain rand, got: {evidence:?}"
    );
    let rand_entry = rand_entry.unwrap();
    assert_eq!(rand_entry["found"], "rand 0.7.3");
    assert!(
        rand_entry["expected"]
            .as_str()
            .unwrap_or("")
            .starts_with("0.")
    );
}

#[test]
fn project_with_no_dependencies_reports_pass() {
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

    assert!(
        output.status.success(),
        "should exit 0 when no deps are outdated"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "pass");
}

#[test]
fn outdated_dep_reports_failure() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "=0.7.3"
"#,
    );

    let output = cargo_bin_cmd!("scute")
        .args(["check", "dependency-freshness"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "should exit non-zero when deps are outdated"
    );
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "fail");
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
