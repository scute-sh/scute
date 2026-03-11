use tempfile::TempDir;

enum ProjectKind {
    Empty,
    Cargo,
    Npm,
}

/// A throwaway project directory for integration tests.
///
/// Use [`cargo()`](Self::cargo) or [`empty()`](Self::empty) to start, chain
/// builder methods, then call [`build()`](Self::build) to materialize the
/// directory. The returned [`TempDir`] is cleaned up on drop.
pub struct TestProject {
    kind: ProjectKind,
    dependencies: Vec<(String, String)>,
    dev_dependencies: Vec<(String, String)>,
    members: Vec<(String, TestMember)>,
    source_files: Vec<(String, String)>,
    scute_config: Option<String>,
}

/// A workspace member inside a [`TestProject`]. Created via [`TestProject::member`].
pub struct TestMember {
    dependencies: Vec<(String, String)>,
    dev_dependencies: Vec<(String, String)>,
}

impl TestMember {
    pub fn dependency(mut self, name: &str, version: &str) -> Self {
        self.dependencies.push((name.into(), version.into()));
        self
    }

    pub fn dev_dependency(mut self, name: &str, version: &str) -> Self {
        self.dev_dependencies.push((name.into(), version.into()));
        self
    }
}

impl TestProject {
    fn new(kind: ProjectKind) -> Self {
        Self {
            kind,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            members: Vec::new(),
            source_files: Vec::new(),
            scute_config: None,
        }
    }

    /// A bare directory with no project scaffolding.
    pub fn empty() -> Self {
        Self::new(ProjectKind::Empty)
    }

    /// A minimal npm project. Runs `npm install` on build to resolve dependencies.
    pub fn npm() -> Self {
        Self::new(ProjectKind::Npm)
    }

    /// A minimal Cargo project with `Cargo.toml` and empty `src/lib.rs`.
    pub fn cargo() -> Self {
        Self::new(ProjectKind::Cargo)
    }

    pub fn dependency(mut self, name: &str, version: &str) -> Self {
        self.dependencies.push((name.into(), version.into()));
        self
    }

    pub fn dev_dependency(mut self, name: &str, version: &str) -> Self {
        self.dev_dependencies.push((name.into(), version.into()));
        self
    }

    /// Add a workspace member. The closure receives an empty [`TestMember`]
    /// to configure with its own dependencies.
    pub fn member(mut self, name: &str, build: impl FnOnce(TestMember) -> TestMember) -> Self {
        let member = build(TestMember {
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        });
        self.members.push((name.into(), member));
        self
    }

    pub fn source_file(mut self, name: &str, content: &str) -> Self {
        self.source_files.push((name.into(), content.into()));
        self
    }

    pub fn scute_config(mut self, yaml: &str) -> Self {
        self.scute_config = Some(yaml.into());
        self
    }

    /// Materialize the project into a temporary directory.
    pub fn build(self) -> TempDir {
        let dir = TempDir::new().unwrap();
        match self.kind {
            ProjectKind::Cargo => setup_cargo_project(
                &dir,
                &self.dependencies,
                &self.dev_dependencies,
                &self.members,
            ),
            ProjectKind::Npm => setup_npm_project(&dir, &self.dependencies, &self.dev_dependencies),
            ProjectKind::Empty => {}
        }
        for (name, content) in &self.source_files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, content).unwrap();
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
    members: &[(String, TestMember)],
) {
    let mut toml = if members.is_empty() {
        String::from(
            "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
    } else {
        let names: Vec<&str> = members.iter().map(|(n, _)| n.as_str()).collect();
        format!(
            "[workspace]\nmembers = [{}]\n",
            names
                .iter()
                .map(|n| format!("\"{n}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    append_cargo_deps(&mut toml, dependencies, dev_dependencies);

    std::fs::write(dir.path().join("Cargo.toml"), toml).unwrap();

    if members.is_empty() {
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("lib.rs"), "").unwrap();
    }

    for (name, member) in members {
        setup_cargo_member(dir, name, &member.dependencies, &member.dev_dependencies);
    }
}

fn setup_cargo_member(
    dir: &TempDir,
    name: &str,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
) {
    let member_dir = dir.path().join(name);
    std::fs::create_dir_all(member_dir.join("src")).unwrap();
    std::fs::write(member_dir.join("src/lib.rs"), "").unwrap();

    let mut toml =
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    append_cargo_deps(&mut toml, dependencies, dev_dependencies);
    std::fs::write(member_dir.join("Cargo.toml"), toml).unwrap();
}

fn setup_npm_project(
    dir: &TempDir,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
) {
    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), "test-project".into());
    pkg.insert("version".into(), "1.0.0".into());

    if !dependencies.is_empty() {
        let deps: serde_json::Map<String, serde_json::Value> = dependencies
            .iter()
            .map(|(n, v)| (n.clone(), serde_json::Value::String(v.clone())))
            .collect();
        pkg.insert("dependencies".into(), deps.into());
    }

    if !dev_dependencies.is_empty() {
        let deps: serde_json::Map<String, serde_json::Value> = dev_dependencies
            .iter()
            .map(|(n, v)| (n.clone(), serde_json::Value::String(v.clone())))
            .collect();
        pkg.insert("devDependencies".into(), deps.into());
    }

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    std::fs::write(dir.path().join("package.json"), json).unwrap();

    let output = std::process::Command::new("npm")
        .args(["install"])
        .current_dir(dir.path())
        .output()
        .expect("npm must be installed to run npm integration tests");

    assert!(
        output.status.success(),
        "npm install failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn append_cargo_deps(
    toml: &mut String,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
) {
    use std::fmt::Write;

    for (section, deps) in [
        ("[dependencies]", dependencies),
        ("[dev-dependencies]", dev_dependencies),
    ] {
        if !deps.is_empty() {
            writeln!(toml, "\n{section}").unwrap();
            for (name, version) in deps {
                writeln!(toml, "{name} = \"{version}\"").unwrap();
            }
        }
    }
}
