use scute_core::dependency_freshness::fetch_outdated;
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
fn only_direct_dependencies_are_reported() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "=0.7.3"
"#,
    );

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1, "should only have direct deps, got: {deps:?}");
    assert_eq!(deps[0].name, "rand");
}

#[test]
fn reports_current_version() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "=0.7.3"
"#,
    );

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].current, "0.7.3");
}

#[test]
fn reports_latest_available_version() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "=0.7.3"
"#,
    );

    let deps = fetch_outdated(dir.path()).unwrap();

    assert!(!deps[0].latest.is_empty());
    assert_ne!(deps[0].latest, deps[0].current);
}

#[test]
fn no_dependencies_returns_empty() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    );

    let deps = fetch_outdated(dir.path()).unwrap();

    assert!(deps.is_empty());
}

#[test]
fn dev_dependencies_are_included() {
    let dir = setup_cargo_project(
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dev-dependencies]
rand = "=0.7.3"
"#,
    );

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "rand");
}
