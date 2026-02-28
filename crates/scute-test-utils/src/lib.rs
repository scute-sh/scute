#![allow(clippy::must_use_candidate, clippy::missing_panics_doc)]

use tempfile::TempDir;

pub fn setup_cargo_project(cargo_toml: &str) -> TempDir {
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

pub fn setup_scute_config(yaml: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".scute.yml"), yaml).unwrap();
    dir
}
