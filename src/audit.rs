use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Display;

use openidea_plugin::{Error, LogLevel, ProcessOutput, host};

const REMEMBERED: usize = 512;
const MAX_LEN: usize = 4000;

thread_local! {
    static SEEN: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

fn emit(level: LogLevel, context: &str, detail: &dyn Display) {
    let mut message = format!("{context}: {detail}");
    if message.len() > MAX_LEN {
        let mut cut = MAX_LEN;
        while !message.is_char_boundary(cut) {
            cut -= 1;
        }
        message.truncate(cut);
        message.push('…');
    }
    let fresh = SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        if seen.len() >= REMEMBERED {
            seen.clear();
        }
        seen.insert(message.clone())
    });
    if fresh {
        host::log(level, &message);
    }
}

pub fn warn(context: &str, detail: impl Display) {
    emit(LogLevel::Warn, context, &detail);
}

pub fn error(context: &str, detail: impl Display) {
    emit(LogLevel::Error, context, &detail);
}

pub fn failed(context: &str, e: &Error) {
    match e {
        Error::Cancelled => {}
        Error::Failed(_) => error(context, e),
        _ => warn(context, e),
    }
}

pub fn tail(text: &str, lines: usize) -> String {
    let kept: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    kept[kept.len().saturating_sub(lines)..].join("\n")
}

pub fn output(out: &ProcessOutput) -> String {
    let status = out.status.map_or("killed by a signal".to_owned(), |s| {
        format!("exit code {s}")
    });
    let detail: Vec<String> = [tail(&out.stderr, 8), tail(&out.stdout, 8)]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if detail.is_empty() {
        format!("{status}, no output")
    } else {
        format!("{status}\n{}", detail.join("\n"))
    }
}

pub trait Logged<T> {
    fn or_warn(self, context: &str) -> Option<T>;
    fn or_error(self, context: &str) -> Option<T>;
}

impl<T, E: Display> Logged<T> for Result<T, E> {
    fn or_warn(self, context: &str) -> Option<T> {
        self.map_err(|e| warn(context, e)).ok()
    }

    fn or_error(self, context: &str) -> Option<T> {
        self.map_err(|e| error(context, e)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn logs_each_distinct_failure_once() {
        let host = MockHost::install();
        warn("tools on save", "exit code 255");
        warn("tools on save", "exit code 255");
        error("mcp sync", "no result");
        failed("command", &Error::Cancelled);
        failed("command", &Error::Failed("boom".into()));
        failed("command", &Error::Denied("process:php".into()));
        assert_eq!(
            host.logs(),
            vec![
                (LogLevel::Warn, "tools on save: exit code 255".to_owned()),
                (LogLevel::Error, "mcp sync: no result".to_owned()),
                (LogLevel::Error, "command: boom".to_owned()),
                (
                    LogLevel::Warn,
                    "command: permission denied: process:php is not granted".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn summarises_process_output() {
        let out = ProcessOutput {
            status: Some(255),
            stdout: "line\n".repeat(20),
            stderr: "PHP Fatal error: Allowed memory size\n".into(),
        };
        let text = output(&out);
        assert!(text.starts_with("exit code 255\nPHP Fatal error"), "{text}");
        assert_eq!(text.lines().count(), 10);
        let none = ProcessOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert_eq!(output(&none), "killed by a signal, no output");
        let _host = MockHost::install();
        assert_eq!(Err::<(), _>("x").or_warn("ctx"), None);
        assert_eq!(Ok::<_, &str>(1).or_error("ctx"), Some(1));
    }
}
