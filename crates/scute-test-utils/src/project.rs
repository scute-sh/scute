use tempfile::TempDir;

enum ProjectKind {
    Empty,
    Cargo,
}

pub struct TestProject {
    kind: ProjectKind,
    dependencies: Vec<(String, String)>,
    dev_dependencies: Vec<(String, String)>,
    members: Vec<(String, TestMember)>,
    source_files: Vec<(String, String)>,
    scute_config: Option<String>,
}

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
    pub fn empty() -> Self {
        Self {
            kind: ProjectKind::Empty,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            members: Vec::new(),
            source_files: Vec::new(),
            scute_config: None,
        }
    }

    pub fn cargo() -> Self {
        Self {
            kind: ProjectKind::Cargo,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            members: Vec::new(),
            source_files: Vec::new(),
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

    pub fn build(self) -> TempDir {
        let dir = TempDir::new().unwrap();
        if matches!(self.kind, ProjectKind::Cargo) {
            setup_cargo_project(
                &dir,
                &self.dependencies,
                &self.dev_dependencies,
                &self.members,
            );
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
    use std::fmt::Write;

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
    use std::fmt::Write;

    let member_dir = dir.path().join(name);
    std::fs::create_dir_all(member_dir.join("src")).unwrap();
    std::fs::write(member_dir.join("src/lib.rs"), "").unwrap();

    let mut toml =
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    if !dependencies.is_empty() {
        toml.push_str("\n[dependencies]\n");
        for (dep_name, version) in dependencies {
            writeln!(toml, "{dep_name} = \"{version}\"").unwrap();
        }
    }
    if !dev_dependencies.is_empty() {
        toml.push_str("\n[dev-dependencies]\n");
        for (dep_name, version) in dev_dependencies {
            writeln!(toml, "{dep_name} = \"{version}\"").unwrap();
        }
    }
    std::fs::write(member_dir.join("Cargo.toml"), toml).unwrap();
}
