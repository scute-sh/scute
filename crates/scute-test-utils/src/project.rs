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
        if let Some(yaml) = &self.scute_config {
            std::fs::write(dir.path().join(".scute.yml"), yaml).unwrap();
        }
        dir
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
