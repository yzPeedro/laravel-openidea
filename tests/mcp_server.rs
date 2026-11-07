use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Server {
    fn start(base: &Path) -> Option<Self> {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("php/mcp.php");
        let mut child = Command::new("php")
            .arg(script)
            .current_dir(base)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = BufReader::new(child.stdout.take()?);
        Some(Self {
            child,
            stdin,
            stdout,
            next: 0,
        })
    }

    fn call(&mut self, method: &str, params: Value) -> Vec<Value> {
        self.next += 1;
        let line = json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params });
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
        let mut out = Vec::new();
        loop {
            let mut text = String::new();
            assert!(
                self.stdout.read_line(&mut text).unwrap() > 0,
                "server closed"
            );
            let message: Value = serde_json::from_str(&text).unwrap();
            let done = message["id"] == json!(self.next);
            out.push(message);
            if done {
                return out;
            }
        }
    }

    fn tools(&mut self) -> (Vec<String>, bool) {
        let out = self.call("tools/list", json!({}));
        let notified = out
            .iter()
            .any(|m| m["method"] == "notifications/tools/list_changed");
        let mut names: Vec<String> = out.last().unwrap()["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect();
        names.sort();
        (names, notified)
    }

    fn tool(&mut self, name: &str, arguments: Value) -> (String, bool) {
        let out = self.call(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &out.last().unwrap()["result"];
        (
            result["content"][0]["text"].as_str().unwrap().to_owned(),
            result["isError"].as_bool().unwrap(),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn project() -> PathBuf {
    let base = std::env::temp_dir().join(format!("openidea-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("storage/openidea")).unwrap();
    std::fs::write(base.join("artisan"), "").unwrap();
    std::fs::write(base.join(".env"), "APP_NAME=x\nAPP_KEY=secret\n").unwrap();
    std::fs::write(base.join(".env.example"), "APP_NAME=\nDB_HOST=\n").unwrap();
    base
}

fn set_level(base: &Path, level: &str) {
    std::fs::write(
        base.join("storage/openidea/mcp-access.json"),
        format!("{{\"level\":\"{level}\"}}\n"),
    )
    .unwrap();
}

#[test]
fn access_levels_decide_what_agents_get() {
    if Command::new("php").arg("-v").output().is_err() {
        eprintln!("php is not installed; skipping");
        return;
    }
    let base = project();
    let mut server = Server::start(&base).unwrap();
    server.call("initialize", json!({ "protocolVersion": "2025-06-18" }));

    let (read, _) = server.tools();
    assert!(read.contains(&"run_artisan".to_owned()));
    assert!(!read.contains(&"format_code".to_owned()));
    assert!(!read.contains(&"run_tinker".to_owned()));

    let (text, error) = server.tool("run_tinker", json!({ "code": "1 + 1" }));
    assert!(error && text.contains("needs full access") && text.contains("MCP access"));
    let (text, error) = server.tool("run_artisan", json!({ "command": "migrate:fresh" }));
    assert!(error && text.contains("needs full access"));
    let (text, error) = server.tool(
        "run_artisan",
        json!({ "command": "make:model", "arguments": ["Post"] }),
    );
    assert!(error && text.contains("needs write access"));
    let (text, error) = server.tool("run_artisan", json!({ "command": "tinker" }));
    assert!(error && text.contains("run_tinker"));

    let (text, error) = server.tool("check_env_keys", json!({}));
    assert!(!error && text.contains("DB_HOST") && !text.contains("secret"));

    set_level(&base, "write");
    let (write, notified) = server.tools();
    assert!(notified);
    assert!(
        write.contains(&"format_code".to_owned()) && write.contains(&"query_to_sql".to_owned())
    );
    assert!(!write.contains(&"run_tinker".to_owned()));

    for code in [
        "Post::query()->pluck('id')->map('system')",
        "Post::query()->getConnection()->getPdo()->exec('drop table posts')",
        "Post::where('id', $id)",
        "Post::where(fn ($q) => $q())",
        "\\exec('id')",
        "Post::where('a', 1); exec('id')",
        "Post::query()->each('Illuminate\\Support\\Facades\\Artisan::call')",
    ] {
        let (text, error) = server.tool("query_to_sql", json!({ "code": code }));
        assert!(error && text.contains("isn't allowed"), "{code}: {text}");
    }
    let (text, error) = server.tool(
        "format_code",
        json!({ "paths": ["storage/openidea/mcp.php"] }),
    );
    assert!(error && text.contains("outside what this tool may touch"));
    let (text, error) = server.tool("format_code", json!({ "paths": ["../etc/passwd"] }));
    assert!(error && text.contains("relative to the project root"));

    set_level(&base, "full");
    let (full, notified) = server.tools();
    assert!(notified && full.contains(&"run_tinker".to_owned()));

    set_level(&base, "root");
    assert_eq!(server.tools().0, read);
    drop(server);
    let _ = std::fs::remove_dir_all(&base);
}
