use crate::Laravel;
use crate::devtools;
use crate::runner;
use openidea_plugin::{LogLevel, Project, host};
use serde_json::{Value, json};

pub const ACCESS_FILE: &str = "storage/openidea/mcp-access.json";

pub fn access(value: Option<&str>) -> &'static str {
    match value {
        Some("write") => "write",
        Some("full") => "full",
        _ => "read",
    }
}

pub fn access_setting() -> &'static str {
    access(host::setting_string("mcpAccess").as_deref())
}

pub fn stored_access(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .get("level")?
        .as_str()
        .map(str::to_owned)
}

pub fn up_to_date(read: &dyn Fn(&str) -> Option<String>, level: &str) -> Option<bool> {
    let server = read("storage/openidea/mcp.php")?;
    Some(
        server == devtools::MCP_PHP
            && read("storage/openidea/install.php").as_deref() == Some(devtools::INSTALL_PHP)
            && read("storage/openidea/recorder.source").as_deref() == Some(devtools::RECORDER_PHP)
            && read(ACCESS_FILE).and_then(|t| stored_access(&t)).as_deref() == Some(level),
    )
}

pub fn payload(command: &str, args: &[String], level: &str) -> String {
    json!({
        "mcp": devtools::MCP_PHP,
        "install": devtools::INSTALL_PHP,
        "recorder": devtools::RECORDER_PHP,
        "access": level,
        "command": command,
        "args": args,
        "guidelines": devtools::GUIDELINES,
    })
    .to_string()
}

impl Laravel {
    pub(crate) fn sync_mcp(&mut self, project: &Project) {
        let Ok(root) = project.root() else {
            return;
        };
        let level = access_setting();
        let read = |f: &str| project.read_file(f).ok();
        if up_to_date(&read, level) != Some(false) {
            return;
        }
        let attempt = format!("{level}:{}", devtools::MCP_PHP.len());
        if self.mcp_synced.get(&root) == Some(&attempt) {
            return;
        }
        self.mcp_synced.insert(root.clone(), attempt);
        let mode = runner::detect(project);
        let (command, args) = mode.mcp_server(&root, runner::compose_file(project));
        let _task = host::begin_task("Laravel: updating the MCP server", false);
        let run = mode.php_args(&[
            "-r".into(),
            devtools::script(devtools::INSTALL_PHP),
            "mcp-sync".into(),
        ]);
        let input = payload(&command, &args, level);
        if let Err(e) = project.run_process(mode.program(), &run, Some(&input)) {
            host::log(
                LogLevel::Warn,
                &format!("couldn't update the MCP server files: {e}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn reads_the_access_level() {
        assert_eq!(access(Some("write")), "write");
        assert_eq!(access(Some("full")), "full");
        assert_eq!(access(Some("admin")), "read");
        assert_eq!(access(None), "read");
        assert_eq!(
            stored_access("{\"level\":\"full\"}\n").as_deref(),
            Some("full")
        );
        assert_eq!(stored_access("nope"), None);
    }

    #[test]
    fn knows_when_the_server_files_are_stale() {
        let mut files = BTreeMap::new();
        let check = |files: &BTreeMap<&str, String>, level| {
            up_to_date(&|f: &str| files.get(f).cloned(), level)
        };
        assert_eq!(check(&files, "read"), None);
        files.insert("storage/openidea/mcp.php", "old".to_owned());
        assert_eq!(check(&files, "read"), Some(false));
        files.insert("storage/openidea/mcp.php", devtools::MCP_PHP.to_owned());
        files.insert(
            "storage/openidea/install.php",
            devtools::INSTALL_PHP.to_owned(),
        );
        files.insert(
            "storage/openidea/recorder.source",
            devtools::RECORDER_PHP.to_owned(),
        );
        files.insert(ACCESS_FILE, "{\"level\":\"read\"}\n".to_owned());
        assert_eq!(check(&files, "read"), Some(true));
        assert_eq!(check(&files, "write"), Some(false));
    }
}
