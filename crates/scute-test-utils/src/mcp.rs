use tempfile::TempDir;

use crate::{Backend, CheckResult, ListChecksResult, target_bin};

pub(crate) struct McpBackend;

impl Backend for McpBackend {
    fn check(&self, _dir: TempDir, _args: &[&str]) -> Box<dyn CheckResult> {
        todo!("MCP check execution")
    }

    fn list_checks(&self, dir: TempDir) -> Box<dyn ListChecksResult> {
        let mut mcp = McpConnection::start(dir.path());
        mcp.initialize();
        let response = mcp.request("tools/list", &serde_json::json!({}));
        let checks = response["result"]["tools"]
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
            .collect();
        Box::new(McpListChecksResult { checks })
    }
}

struct McpListChecksResult {
    checks: Vec<String>,
}

impl ListChecksResult for McpListChecksResult {
    fn expect_contains(&self, name: &str) -> &dyn ListChecksResult {
        assert!(
            self.checks.iter().any(|c| c == name),
            "expected check '{name}' in {:?}",
            self.checks
        );
        self
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
