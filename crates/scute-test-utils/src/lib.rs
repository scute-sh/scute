#![allow(
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::return_self_not_must_use
)]

use tempfile::TempDir;

#[derive(Default)]
pub struct TestProject {
    cargo_toml: Option<String>,
    scute_config: Option<String>,
}

impl TestProject {
    pub fn new() -> Self {
        Self {
            cargo_toml: None,
            scute_config: None,
        }
    }

    pub fn cargo_toml(mut self, toml: &str) -> Self {
        self.cargo_toml = Some(toml.into());
        self
    }

    pub fn scute_config(mut self, yaml: &str) -> Self {
        self.scute_config = Some(yaml.into());
        self
    }

    pub fn build(self) -> TempDir {
        let dir = TempDir::new().unwrap();
        if let Some(toml) = &self.cargo_toml {
            std::fs::write(dir.path().join("Cargo.toml"), toml).unwrap();
            let src = dir.path().join("src");
            std::fs::create_dir(&src).unwrap();
            std::fs::write(src.join("lib.rs"), "").unwrap();
            std::process::Command::new("cargo")
                .args(["generate-lockfile"])
                .current_dir(dir.path())
                .output()
                .unwrap();
        }
        if let Some(yaml) = &self.scute_config {
            std::fs::write(dir.path().join(".scute.yml"), yaml).unwrap();
        }
        dir
    }
}
