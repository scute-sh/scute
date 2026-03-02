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
    Mcp,
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

    pub fn mcp() -> Self {
        Self::with_mode(ScuteMode::Mcp)
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

    pub fn list_checks(self) -> Vec<String> {
        assert!(
            matches!(self.mode, ScuteMode::Mcp),
            "list_checks is only supported in MCP mode"
        );
        let dir = self.project.build();
        let mut mcp = McpConnection::start(dir.path());
        mcp.initialize();
        let response = mcp.request("tools/list", &serde_json::json!({}));
        response["result"]["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|t| {
                t["name"]
                    .as_str()
                    .expect("tool name")
                    .strip_prefix("check_")
                    .expect("tool name starts with check_")
                    .replace('_', "-")
            })
            .collect()
    }

    pub fn check(self, args: &[&str]) -> ScuteResult {
        let mut full_args = vec!["check"];
        full_args.extend_from_slice(args);
        self.execute(&full_args)
    }

    fn execute(self, args: &[&str]) -> ScuteResult {
        let dir = self.project.build();
        let bin = target_bin("scute");
        let mut cmd = assert_cmd::Command::new(&bin);
        cmd.current_dir(dir.path());
        match self.mode {
            ScuteMode::Cli => {
                cmd.args(args);
            }
            ScuteMode::CliStdin => {
                let message = args.last().expect("CliStdin requires message in args");
                cmd.args(&args[..args.len() - 1])
                    .write_stdin(message.to_string());
            }
            ScuteMode::Mcp => {
                todo!("MCP check execution")
            }
        }
        let output = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        ScuteResult {
            dir,
            exit_code: output.status.code().unwrap_or(-1),
            json: serde_json::from_str(&stdout).ok(),
            stderr,
        }
    }
}

pub struct ScuteResult {
    dir: TempDir,
    exit_code: i32,
    json: Option<serde_json::Value>,
    stderr: String,
}

impl ScuteResult {
    pub fn expect_check_pass(&self) -> &Self {
        let evaluation = &self.json()["evaluation"];
        assert_eq!(evaluation["status"], "pass", "got: {}", self.json());
        assert_eq!(
            self.exit_code, 0,
            "expected exit 0, stderr: {}",
            self.stderr
        );
        self
    }

    pub fn expect_check_warn(&self) -> &Self {
        let evaluation = &self.json()["evaluation"];
        assert_eq!(evaluation["status"], "warn", "got: {}", self.json());
        assert_eq!(
            self.exit_code, 0,
            "expected exit 0 for warn, stderr: {}",
            self.stderr
        );
        self
    }

    pub fn expect_check_fail(&self) -> &Self {
        let evaluation = &self.json()["evaluation"];
        assert_eq!(evaluation["status"], "fail", "got: {}", self.json());
        assert_eq!(self.exit_code, 1, "expected exit 1 for fail");
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
        assert_eq!(
            self.json()["evaluation"]["measurement"]["observed"],
            expected
        );
        self
    }

    pub fn expect_evidence_rule(&self, index: usize, rule: &str) -> &Self {
        assert_eq!(self.json()["evaluation"]["evidence"][index]["rule"], rule);
        self
    }

    pub fn expect_evidence_has_expected(&self, index: usize) -> &Self {
        assert!(
            !self.json()["evaluation"]["evidence"][index]["expected"].is_null(),
            "expected evidence[{index}].expected to be present"
        );
        self
    }

    pub fn expect_evidence_no_expected(&self, index: usize) -> &Self {
        assert!(
            self.json()["evaluation"]["evidence"][index]
                .get("expected")
                .is_none(),
            "expected evidence[{index}].expected to be absent"
        );
        self
    }

    pub fn expect_no_evidences(&self) -> &Self {
        assert!(
            self.json()["evaluation"].get("evidence").is_none(),
            "expected evidence key to be absent, got: {}",
            self.json()["evaluation"]
        );
        self
    }

    pub fn debug(&self) -> &Self {
        eprintln!("exit_code: {}", self.exit_code);
        eprintln!(
            "stdout: {}",
            self.json
                .as_ref()
                .map_or("(none)".into(), std::string::ToString::to_string)
        );
        eprintln!("stderr: {}", self.stderr);
        self
    }

    pub fn expect_error(&self, code: &str) -> &Self {
        let error = &self.json()["error"];
        assert_eq!(error["code"], code, "got: {}", self.json());
        assert!(
            error["message"].is_string(),
            "error.message should be present"
        );
        assert!(
            error["recovery"].is_string(),
            "error.recovery should be present"
        );
        assert_eq!(self.exit_code, 2, "expected exit 2 for error");
        self
    }

    fn json(&self) -> &serde_json::Value {
        self.json
            .as_ref()
            .expect("expected valid JSON in stdout, got none")
    }
}

struct McpConnection {
    child: std::process::Child,
    reader: std::io::BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpConnection {
    fn start(working_dir: &std::path::Path) -> Self {
        use std::process::{Command, Stdio};

        let mut child = Command::new(target_bin("scute-mcp"))
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start scute-mcp");

        let stdout = child.stdout.take().expect("stdout");
        let reader = std::io::BufReader::new(stdout);

        Self {
            child,
            reader,
            next_id: 0,
        }
    }

    fn initialize(&mut self) {
        self.request(
            "initialize",
            &serde_json::json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "scute-test", "version": "0.0.0" }
            }),
        );
        self.notify("notifications/initialized", &serde_json::json!({}));
    }

    fn request(&mut self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        use std::io::{BufRead, Write};

        self.next_id += 1;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": method,
            "params": params,
        });

        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();

        let mut line = String::new();
        self.reader.read_line(&mut line).expect("read response");

        serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("invalid JSON from MCP server: {e}\nraw: {line}"))
    }

    fn notify(&mut self, method: &str, params: &serde_json::Value) {
        use std::io::Write;

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }
}

impl Drop for McpConnection {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn target_bin(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::current_exe().expect("need current_exe for binary lookup");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
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
