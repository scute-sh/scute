use std::path::Path;

use tempfile::TempDir;

enum ProjectKind {
    Empty,
    Cargo,
    Npm,
    Pnpm,
}

/// A throwaway project directory for integration tests.
///
/// Use [`cargo()`](Self::cargo), [`npm()`](Self::npm), or
/// [`empty()`](Self::empty) to start, chain builder methods, then call
/// [`build()`](Self::build) to materialize the directory. The returned
/// [`TempDir`] is cleaned up on drop.
pub struct TestProject {
    kind: ProjectKind,
    dependencies: Vec<(String, String)>,
    dev_dependencies: Vec<(String, String)>,
    members: Vec<(String, TestMember)>,
    children: Vec<(String, TestProject)>,
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
            children: Vec::new(),
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

    /// A minimal pnpm project. Runs `pnpm install` on build to resolve dependencies.
    pub fn pnpm() -> Self {
        Self::new(ProjectKind::Pnpm)
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

    /// Add a workspace member at the given relative path. The closure receives
    /// an empty [`TestMember`] to configure with its own dependencies.
    pub fn member(mut self, path: &str, build: impl FnOnce(TestMember) -> TestMember) -> Self {
        let member = build(TestMember {
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        });
        self.members.push((path.into(), member));
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

    /// Nest a child project at the given relative path inside this project.
    pub fn nested(mut self, path: &str, child: TestProject) -> Self {
        self.children.push((path.into(), child));
        self
    }

    /// Materialize the project into a temporary directory.
    ///
    /// The directory is initialized as a git repo with a `.gitignore`
    /// that excludes `node_modules/` and `target/`, matching what any
    /// real project would have.
    pub fn build(self) -> TempDir {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());
        self.setup_at(dir.path());
        dir
    }

    fn setup_at(self, root: &Path) {
        std::fs::create_dir_all(root).unwrap();
        match self.kind {
            ProjectKind::Cargo => setup_cargo_project(
                root,
                &self.dependencies,
                &self.dev_dependencies,
                &self.members,
            ),
            ProjectKind::Npm => setup_npm_project(
                root,
                &self.dependencies,
                &self.dev_dependencies,
                &self.members,
            ),
            ProjectKind::Pnpm => setup_pnpm_project(
                root,
                &self.dependencies,
                &self.dev_dependencies,
                &self.members,
            ),
            ProjectKind::Empty => {}
        }
        for (name, content) in &self.source_files {
            let path = root.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, content).unwrap();
        }
        if let Some(yaml) = &self.scute_config {
            std::fs::write(root.join(".scute.yml"), yaml).unwrap();
        }
        for (path, child) in self.children {
            child.setup_at(&root.join(path));
        }
    }
}

fn setup_cargo_project(
    root: &Path,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
    members: &[(String, TestMember)],
) {
    let mut toml = if members.is_empty() {
        String::from(
            "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
    } else {
        format!(
            "[workspace]\nmembers = [{}]\n",
            members
                .iter()
                .map(|(path, _)| format!("\"{path}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    append_cargo_deps(&mut toml, dependencies, dev_dependencies);

    std::fs::write(root.join("Cargo.toml"), toml).unwrap();

    if members.is_empty() {
        let src = root.join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("lib.rs"), "").unwrap();
    }

    for (path, member) in members {
        setup_cargo_member(root, path, member);
    }
}

fn setup_cargo_member(root: &Path, path: &str, member: &TestMember) {
    let member_dir = root.join(path);
    std::fs::create_dir_all(member_dir.join("src")).unwrap();
    std::fs::write(member_dir.join("src/lib.rs"), "").unwrap();

    let name = Path::new(path).file_name().unwrap().to_string_lossy();
    let mut toml =
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    append_cargo_deps(&mut toml, &member.dependencies, &member.dev_dependencies);
    std::fs::write(member_dir.join("Cargo.toml"), toml).unwrap();
}

fn setup_npm_project(
    root: &Path,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
    members: &[(String, TestMember)],
) {
    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), "test-project".into());
    pkg.insert("version".into(), "1.0.0".into());

    if !members.is_empty() {
        let workspace_paths: Vec<serde_json::Value> = members
            .iter()
            .map(|(path, _)| serde_json::Value::String(path.clone()))
            .collect();
        pkg.insert("workspaces".into(), workspace_paths.into());
    }

    append_npm_deps(&mut pkg, dependencies, dev_dependencies);

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    std::fs::write(root.join("package.json"), json).unwrap();

    for (path, member) in members {
        setup_npm_member(root, path, member);
    }

    let output = std::process::Command::new("npm")
        .args(["install"])
        .current_dir(root)
        .output()
        .expect("npm must be installed to run npm integration tests");

    assert!(
        output.status.success(),
        "npm install failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup_npm_member(root: &Path, path: &str, member: &TestMember) {
    let member_dir = root.join(path);
    std::fs::create_dir_all(&member_dir).unwrap();

    let basename = Path::new(path).file_name().unwrap().to_string_lossy();

    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), format!("@test/{basename}").into());
    pkg.insert("version".into(), "1.0.0".into());

    append_npm_deps(&mut pkg, &member.dependencies, &member.dev_dependencies);

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    std::fs::write(member_dir.join("package.json"), json).unwrap();
}

fn setup_pnpm_project(
    root: &Path,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
    members: &[(String, TestMember)],
) {
    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), "test-project".into());
    pkg.insert("version".into(), "1.0.0".into());

    append_npm_deps(&mut pkg, dependencies, dev_dependencies);

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    std::fs::write(root.join("package.json"), json).unwrap();

    if !members.is_empty() {
        let patterns: Vec<String> = members.iter().map(|(path, _)| path.clone()).collect();
        let yaml = format!(
            "packages:\n{}",
            patterns
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        std::fs::write(root.join("pnpm-workspace.yaml"), yaml).unwrap();

        for (path, member) in members {
            setup_pnpm_member(root, path, member);
        }
    }

    let output = std::process::Command::new("pnpm")
        .args(["install"])
        .current_dir(root)
        .output()
        .expect("pnpm must be installed to run pnpm integration tests");

    assert!(
        output.status.success(),
        "pnpm install failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup_pnpm_member(root: &Path, path: &str, member: &TestMember) {
    let member_dir = root.join(path);
    std::fs::create_dir_all(&member_dir).unwrap();

    let basename = Path::new(path).file_name().unwrap().to_string_lossy();

    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), format!("@test/{basename}").into());
    pkg.insert("version".into(), "1.0.0".into());

    append_npm_deps(&mut pkg, &member.dependencies, &member.dev_dependencies);

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    std::fs::write(member_dir.join("package.json"), json).unwrap();
}

fn append_npm_deps(
    pkg: &mut serde_json::Map<String, serde_json::Value>,
    dependencies: &[(String, String)],
    dev_dependencies: &[(String, String)],
) {
    for (key, deps) in [
        ("dependencies", dependencies),
        ("devDependencies", dev_dependencies),
    ] {
        if !deps.is_empty() {
            let map: serde_json::Map<String, serde_json::Value> = deps
                .iter()
                .map(|(n, v)| (n.clone(), serde_json::Value::String(v.clone())))
                .collect();
            pkg.insert(key.into(), map.into());
        }
    }
}

fn init_git_repo(root: &Path) {
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".gitignore"), "node_modules/\ntarget/\n").unwrap();
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
