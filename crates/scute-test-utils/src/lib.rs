#![allow(
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::return_self_not_must_use
)]

use tempfile::TempDir;

enum ProjectKind {
    Empty,
    Cargo,
}

pub struct TestProject {
    kind: ProjectKind,
    dependencies: Vec<(String, String)>,
    dev_dependencies: Vec<(String, String)>,
    scute_config: Option<String>,
}

impl TestProject {
    pub fn empty() -> Self {
        Self {
            kind: ProjectKind::Empty,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            scute_config: None,
        }
    }

    pub fn cargo() -> Self {
        Self {
            kind: ProjectKind::Cargo,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            scute_config: None,
        }
    }

    pub fn dependency(mut self, name: &str, version: &str) -> Self {
        self.dependencies.push((name.into(), version.into()));
        self
    }

    pub fn dev_dependency(mut self, name: &str, version: &str) -> Self {
        self.dev_dependencies.push((name.into(), version.into()));
        self
    }

    pub fn scute_config(mut self, yaml: &str) -> Self {
        self.scute_config = Some(yaml.into());
        self
    }

    pub fn build(self) -> TempDir {
        let dir = TempDir::new().unwrap();
        if matches!(self.kind, ProjectKind::Cargo) {
            setup_cargo_project(&dir, &self.dependencies, &self.dev_dependencies);
        }
        write_scute_config(&dir, self.scute_config.as_ref());
        dir
    }
}

pub enum ScuteMode {
    Cli,
    CliStdin,
}

pub struct Scute {
    mode: ScuteMode,
    project: TestProject,
}

impl Scute {
    fn with_mode(mode: ScuteMode) -> Self {
        Self {
            mode,
            project: TestProject::cargo(),
        }
    }

    pub fn cli() -> Self {
        Self::with_mode(ScuteMode::Cli)
    }

    pub fn cli_stdin() -> Self {
        Self::with_mode(ScuteMode::CliStdin)
    }

    pub fn dependency(mut self, name: &str, version: &str) -> Self {
        self.project = self.project.dependency(name, version);
        self
    }

    pub fn dev_dependency(mut self, name: &str, version: &str) -> Self {
        self.project = self.project.dev_dependency(name, version);
        self
    }

    pub fn scute_config(mut self, yaml: &str) -> Self {
        self.project = self.project.scute_config(yaml);
        self
    }

    pub fn check(self, args: &[&str]) -> ScuteResult {
        let dir = TempDir::new().unwrap();
        setup_cargo_project(
            &dir,
            &self.project.dependencies,
            &self.project.dev_dependencies,
        );
        write_scute_config(&dir, self.project.scute_config.as_ref());

        let bin = scute_bin();
        let mut cmd = assert_cmd::Command::new(&bin);
        cmd.arg("check").args(args).current_dir(dir.path());
        if matches!(self.mode, ScuteMode::CliStdin) {
            let message = args.last().expect("CliStdin requires message in args");
            cmd = assert_cmd::Command::new(&bin);
            cmd.arg("check")
                .args(&args[..args.len() - 1])
                .current_dir(dir.path())
                .write_stdin(message.to_string());
        }
        let output = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        ScuteResult {
            dir,
            exit_success: output.status.success(),
            json: serde_json::from_str(&stdout).ok(),
            stderr,
        }
    }
}

pub struct ScuteResult {
    dir: TempDir,
    exit_success: bool,
    json: Option<serde_json::Value>,
    stderr: String,
}

impl ScuteResult {
    pub fn expect_pass(&self) -> &Self {
        let json = self.json();
        assert_eq!(json["status"], "pass", "got: {json}");
        assert!(
            self.exit_success,
            "expected exit 0, stderr: {}",
            self.stderr
        );
        self
    }

    pub fn expect_warn(&self) -> &Self {
        let json = self.json();
        assert_eq!(json["status"], "warn", "got: {json}");
        assert!(
            self.exit_success,
            "expected exit 0 for warn, stderr: {}",
            self.stderr
        );
        self
    }

    pub fn expect_fail(&self) -> &Self {
        let json = self.json();
        assert_eq!(json["status"], "fail", "got: {json}");
        assert!(!self.exit_success, "expected non-zero exit");
        self
    }

    pub fn expect_target(&self, expected: &str) -> &Self {
        assert_eq!(self.json()["target"], expected);
        self
    }

    pub fn expect_target_matches_dir(&self) -> &Self {
        let target = self.json()["target"]
            .as_str()
            .expect("target should be a string");
        assert_eq!(
            std::path::Path::new(target).canonicalize().unwrap(),
            self.dir.path().canonicalize().unwrap()
        );
        self
    }

    pub fn expect_observed(&self, expected: u64) -> &Self {
        assert_eq!(self.json()["measurement"]["observed"], expected);
        self
    }

    pub fn expect_evidence_rule(&self, index: usize, rule: &str) -> &Self {
        assert_eq!(self.json()["evidence"][index]["rule"], rule);
        self
    }

    pub fn expect_evidence_has_expected(&self, index: usize) -> &Self {
        assert!(
            !self.json()["evidence"][index]["expected"].is_null(),
            "expected evidence[{index}].expected to be present"
        );
        self
    }

    pub fn expect_evidence_no_expected(&self, index: usize) -> &Self {
        assert!(
            self.json()["evidence"][index].get("expected").is_none(),
            "expected evidence[{index}].expected to be absent"
        );
        self
    }

    pub fn expect_error_containing(&self, needle: &str) -> &Self {
        assert!(!self.exit_success, "expected non-zero exit");
        assert!(
            self.stderr.contains(needle),
            "expected stderr to contain {needle:?}, got: {}",
            self.stderr
        );
        self
    }

    fn json(&self) -> &serde_json::Value {
        self.json
            .as_ref()
            .expect("expected valid JSON in stdout, got none")
    }
}

fn write_scute_config(dir: &TempDir, config: Option<&String>) {
    if let Some(yaml) = config {
        std::fs::write(dir.path().join(".scute.yml"), yaml).unwrap();
    }
}

fn setup_cargo_project(
    dir: &TempDir,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
) {
    use std::fmt::Write;

    let mut toml = String::from(
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    if !dependencies.is_empty() {
        toml.push_str("\n[dependencies]\n");
        for (name, version) in dependencies {
            writeln!(toml, "{name} = \"{version}\"").unwrap();
        }
    }
    if !dev_dependencies.is_empty() {
        toml.push_str("\n[dev-dependencies]\n");
        for (name, version) in dev_dependencies {
            writeln!(toml, "{name} = \"{version}\"").unwrap();
        }
    }
    std::fs::write(dir.path().join("Cargo.toml"), toml).unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir(&src).unwrap();
    std::fs::write(src.join("lib.rs"), "").unwrap();

    if !dependencies.is_empty() || !dev_dependencies.is_empty() {
        std::process::Command::new("cargo")
            .args(["generate-lockfile"])
            .current_dir(dir.path())
            .output()
            .unwrap();
    }
}

fn scute_bin() -> std::path::PathBuf {
    let mut dir = std::env::current_exe().expect("need current_exe for binary lookup");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir.join(format!("scute{}", std::env::consts::EXE_SUFFIX))
}
