use tempfile::TempDir;

use crate::{Backend, CheckResult, ListChecksResult, target_bin};

pub(crate) struct McpBackend;

impl Backend for McpBackend {
    fn check(&self, dir: TempDir, args: &[&str]) -> Box<dyn CheckResult> {
        let check_name = args.get(1).expect("check name required");
        let tool_name = format!("check_{}", check_name.replace('-', "_"));
        let tool_args = build_tool_args(check_name, &args[2..]);

        let mut mcp = McpConnection::start(dir.path());
        mcp.initialize();
        let response = mcp.request(
            "tools/call",
            &serde_json::json!({
                "name": tool_name,
                "arguments": tool_args,
            }),
        );

        let json = response["result"]["structuredContent"].clone();
        Box::new(McpCheckResult { json })
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

fn build_tool_args(check_name: &str, args: &[&str]) -> serde_json::Value {
    match check_name {
        "commit-message" => {
            let message = args.first().copied().unwrap_or("");
            serde_json::json!({ "message": message })
        }
        _ => serde_json::json!({}),
    }
}

struct McpCheckResult {
    json: serde_json::Value,
}

impl CheckResult for McpCheckResult {
    fn expect_pass(&self) -> &dyn CheckResult {
        assert_eq!(
            self.json["evaluation"]["status"], "pass",
            "got: {}",
            self.json
        );
        self
    }

    fn expect_warn(&self) -> &dyn CheckResult {
        assert_eq!(
            self.json["evaluation"]["status"], "warn",
            "got: {}",
            self.json
        );
        self
    }

    fn expect_fail(&self) -> &dyn CheckResult {
        assert_eq!(
            self.json["evaluation"]["status"], "fail",
            "got: {}",
            self.json
        );
        self
    }

    fn expect_target(&self, expected: &str) -> &dyn CheckResult {
        assert_eq!(self.json["target"], expected);
        self
    }

    fn expect_target_matches_dir(&self) -> &dyn CheckResult {
        todo!("MCP target dir matching")
    }

    fn expect_observed(&self, expected: u64) -> &dyn CheckResult {
        assert_eq!(self.json["evaluation"]["measurement"]["observed"], expected);
        self
    }

    fn expect_evidence_rule(&self, index: usize, rule: &str) -> &dyn CheckResult {
        assert_eq!(self.json["evaluation"]["evidence"][index]["rule"], rule);
        self
    }

    fn expect_evidence_has_expected(&self, index: usize) -> &dyn CheckResult {
        assert!(
            !self.json["evaluation"]["evidence"][index]["expected"].is_null(),
            "expected evidence[{index}].expected to be present"
        );
        self
    }

    fn expect_evidence_no_expected(&self, index: usize) -> &dyn CheckResult {
        assert!(
            self.json["evaluation"]["evidence"][index]
                .get("expected")
                .is_none(),
            "expected evidence[{index}].expected to be absent"
        );
        self
    }

    fn expect_no_evidences(&self) -> &dyn CheckResult {
        assert!(
            self.json["evaluation"].get("evidence").is_none(),
            "expected evidence key to be absent, got: {}",
            self.json["evaluation"]
        );
        self
    }

    fn expect_error(&self, code: &str) -> &dyn CheckResult {
        let error = &self.json["error"];
        assert_eq!(error["code"], code, "got: {}", self.json);
        assert!(
            error["message"].is_string(),
            "error.message should be present"
        );
        assert!(
            error["recovery"].is_string(),
            "error.recovery should be present"
        );
        self
    }

    fn debug(&self) -> &dyn CheckResult {
        eprintln!("json: {}", self.json);
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
