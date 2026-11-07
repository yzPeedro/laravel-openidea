use crate::Laravel;
use crate::audit;
use crate::devtools;
use crate::project::rel;
use crate::runner::{self, Mode};
use openidea_plugin::{Diagnostic, NotificationLevel, Position, Project, Range, Severity, host};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formatter {
    Pint,
    PhpCsFixer,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JobResult {
    pub name: String,
    pub code: Option<i32>,
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub timeout: bool,
}

pub fn failure(result: &JobResult) -> Option<String> {
    if result.timeout {
        return None;
    }
    let failed = if result.name == "phpstan" {
        !result.stdout.contains("\"totals\"")
    } else {
        result.code != Some(0)
    };
    if !failed {
        return None;
    }
    let output = [result.stderr.trim(), result.stdout.trim()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(6)..].join(
        "
",
    );
    Some(match (tail.is_empty(), result.code) {
        (true, Some(code)) => format!("It exited with code {code} and printed nothing."),
        (true, None) => "It didn't start.".to_owned(),
        (false, _) => tail,
    })
}

pub fn formatter_setting(value: Option<&str>) -> Option<Formatter> {
    match value {
        Some("pint") => Some(Formatter::Pint),
        Some("php-cs-fixer") => Some(Formatter::PhpCsFixer),
        _ => None,
    }
}

pub fn wants_tools(rel_path: &str) -> bool {
    let p = rel_path.replace('\\', "/");
    p.ends_with(".php")
        && !p.ends_with(".blade.php")
        && !["vendor/", "storage/", "bootstrap/cache/", "node_modules/"]
            .iter()
            .any(|d| p.starts_with(d))
}

pub fn jobs(
    rel_path: &str,
    formatter: Option<Formatter>,
    phpstan: bool,
    has: &dyn Fn(&str) -> bool,
) -> Vec<Value> {
    let mut out = Vec::new();
    match formatter {
        Some(Formatter::Pint) if has("vendor/bin/pint") => {
            out.push(json!({ "name": "pint", "argv": ["vendor/bin/pint", rel_path, "--no-interaction"] }));
        }
        Some(Formatter::PhpCsFixer) if has("vendor/bin/php-cs-fixer") => {
            out.push(json!({ "name": "php-cs-fixer", "argv": ["vendor/bin/php-cs-fixer", "fix", rel_path, "--quiet", "--no-interaction"] }));
        }
        _ => {}
    }
    let configured = ["phpstan.neon", "phpstan.neon.dist", "phpstan.dist.neon"]
        .iter()
        .any(|f| has(f));
    if phpstan && configured && has("vendor/bin/phpstan") {
        out.push(json!({
            "name": "phpstan",
            "argv": ["vendor/bin/phpstan", "analyse", "--error-format=json", "--no-progress", "--no-interaction", "--memory-limit=1G", rel_path],
        }));
    }
    out
}

pub fn parse_results(stdout: &str) -> Vec<JobResult> {
    stdout
        .rfind("@@openidea@@")
        .and_then(|i| serde_json::from_str(stdout[i + 12..].trim()).ok())
        .unwrap_or_default()
}

pub fn phpstan_messages(stdout: &str, rel_path: &str) -> Vec<(u32, String, Option<String>)> {
    let Some(start) = stdout.find('{') else {
        return Vec::new();
    };
    let Ok(report) = serde_json::from_str::<Value>(&stdout[start..]) else {
        return Vec::new();
    };
    let Some(files) = report.get("files").and_then(Value::as_object) else {
        return Vec::new();
    };
    let wanted = rel_path.replace('\\', "/");
    files
        .iter()
        .filter(|(path, _)| path.replace('\\', "/").ends_with(&wanted))
        .flat_map(|(_, file)| {
            file.get("messages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|m| {
            let line = m.get("line").and_then(Value::as_u64).unwrap_or(1).max(1) as u32;
            let mut message = m.get("message")?.as_str()?.to_owned();
            if let Some(tip) = m.get("tip").and_then(Value::as_str) {
                message.push_str(&format!(" ({tip})"));
            }
            let identifier = m
                .get("identifier")
                .and_then(Value::as_str)
                .map(str::to_owned);
            Some((line - 1, message, identifier))
        })
        .collect()
}

const SAVE_SECONDS: f64 = 15.0;

impl Laravel {
    pub(crate) fn tools_on_save(&mut self, project: &Project, root: &str, path: &str) {
        let rel_path = rel(root, path).replace('\\', "/");
        if !wants_tools(&rel_path) {
            return;
        }
        let formatter = formatter_setting(host::setting_string("formatOnSave").as_deref());
        let phpstan = host::setting_bool("phpstanOnSave").unwrap_or(false);
        if formatter.is_none() && !phpstan {
            self.analysis.remove(path);
            return;
        }
        let has = |f: &str| project.read_file(f).is_ok();
        let jobs = jobs(&rel_path, formatter, phpstan, &has);
        if jobs.is_empty() {
            return;
        }
        let names: Vec<&str> = jobs.iter().filter_map(|j| j["name"].as_str()).collect();
        let label = names.join(" and ");
        let _task = host::begin_task(&format!("Laravel: {label}"), false);
        let mode = runner::detect(project);
        let tmp = matches!(mode, Mode::DockerRun(_)).then_some("storage/framework/cache/openidea");
        let input = json!({ "seconds": SAVE_SECONDS, "jobs": jobs, "tmp": tmp }).to_string();
        let args = mode.php_args(&["-r".into(), devtools::script(devtools::TOOLS_PHP)]);
        let out = match project.run_process(mode.program(), &args, Some(&input)) {
            Ok(out) => out,
            Err(e) => {
                audit::failed(&format!("{label} on save"), &e);
                self.warn_once(
                    "tools",
                    "Laravel OpenIDEA couldn't run the tools on save",
                    &e.to_string(),
                );
                return;
            }
        };
        let results = parse_results(&out.stdout);
        if results.is_empty() {
            audit::error(
                &format!("{label} on save"),
                format!("no results for {rel_path}: {}", audit::output(&out)),
            );
        }
        for result in results {
            if result.timeout {
                audit::warn(
                    &format!("{} on save", result.name),
                    format!("timed out after 15 seconds on {rel_path}"),
                );
                self.warn_once(
                    &result.name,
                    &format!("{} took longer than 15 seconds", result.name),
                    "It was stopped after 15 seconds. Run it from the terminal for the full result; later saves are usually faster thanks to its cache.",
                );
                continue;
            }
            if let Some(detail) = failure(&result) {
                audit::warn(
                    &format!("{} on save", result.name),
                    format!("failed on {rel_path}: {detail}"),
                );
                self.warn_once(
                    &format!("{}-failed", result.name),
                    &format!("{} failed on save", result.name),
                    &detail,
                );
                if result.name == "phpstan" {
                    self.analysis.remove(path);
                }
                continue;
            }
            if result.name != "phpstan" {
                continue;
            }
            let text = project.read_file(path).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            let diagnostics = phpstan_messages(&result.stdout, &rel_path)
                .into_iter()
                .map(|(line, message, identifier)| {
                    let width = lines
                        .get(line as usize)
                        .map_or(0, |l| l.encode_utf16().count() as u32);
                    let indent = lines
                        .get(line as usize)
                        .map_or(0, |l| (l.len() - l.trim_start().len()) as u32);
                    let range = Range {
                        start: Position {
                            line,
                            character: indent,
                        },
                        end: Position {
                            line,
                            character: width,
                        },
                    };
                    let mut d = Diagnostic::new(range, Severity::Warning, message);
                    d.source = Some("PHPStan".into());
                    d.code = identifier;
                    d
                })
                .collect();
            self.analysis.insert(path.to_owned(), diagnostics);
        }
    }

    fn warn_once(&mut self, key: &str, title: &str, body: &str) {
        if self.warned.insert(key.to_owned()) {
            host::notify(NotificationLevel::Warning, title, body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_jobs_the_project_can_run() {
        let all = |_: &str| true;
        let names = |j: Vec<Value>| {
            j.iter()
                .map(|x| x["name"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(jobs("app/A.php", Some(Formatter::Pint), true, &all)),
            ["pint", "phpstan"]
        );
        assert_eq!(
            names(jobs("app/A.php", Some(Formatter::PhpCsFixer), false, &all)),
            ["php-cs-fixer"]
        );
        let no_config = |f: &str| !f.starts_with("phpstan");
        assert!(jobs("app/A.php", None, true, &no_config).is_empty());
        assert!(!wants_tools("vendor/x/A.php"));
        assert!(!wants_tools("resources/views/a.blade.php"));
        assert!(wants_tools("app/Models/User.php"));
        assert_eq!(formatter_setting(Some("off")), None);
    }

    #[test]
    fn reads_phpstan_json_for_the_saved_file() {
        let stdout = r#"{"totals":{"errors":0,"file_errors":2},"files":{"/var/www/html/app/Models/User.php":{"errors":2,"messages":[{"message":"Method App\\Models\\User::name() has no return type specified.","line":12,"ignorable":true,"identifier":"missingType.return"},{"message":"Undefined variable: $x","line":20,"ignorable":true,"tip":"Learn more"}]}},"errors":[]}"#;
        let found = phpstan_messages(stdout, "app/Models/User.php");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, 11);
        assert_eq!(found[0].2.as_deref(), Some("missingType.return"));
        assert!(found[1].1.ends_with("(Learn more)"));
        let wrapped = format!(
            "noise\n@@openidea@@{}\n",
            r#"[{"name":"phpstan","code":1,"stdout":"{}","stderr":"","timeout":false}]"#
        );
        assert_eq!(parse_results(&wrapped)[0].name, "phpstan");
    }

    #[test]
    fn tells_a_failed_tool_from_one_that_found_problems() {
        let job = |name: &str, code: Option<i32>, stdout: &str, stderr: &str| JobResult {
            name: name.into(),
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
            timeout: false,
        };
        assert!(failure(&job("phpstan", Some(1), r#"{"totals":{}}"#, "Note: config")).is_none());
        assert_eq!(
            failure(&job(
                "phpstan",
                Some(255),
                "",
                "PHP Fatal error: Allowed memory size
"
            ))
            .as_deref(),
            Some("PHP Fatal error: Allowed memory size")
        );
        assert!(failure(&job("pint", Some(0), "  ✓ 1 file", "")).is_none());
        assert_eq!(
            failure(&job("pint", Some(1), "", "")).as_deref(),
            Some("It exited with code 1 and printed nothing.")
        );
        assert_eq!(
            failure(&job("pint", None, "", "")).as_deref(),
            Some("It didn't start.")
        );
    }
}
