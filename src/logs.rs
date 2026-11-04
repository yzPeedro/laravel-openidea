use openidea_plugin::Project;
use serde::Serialize;
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 2000;
const DAILY_DAYS: i64 = 14;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogEntry {
    pub time: String,
    pub env: String,
    pub level: String,
    pub message: String,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogFile {
    pub path: String,
    pub size: usize,
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn recent_dates() -> Vec<String> {
    let today = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64 / 86_400);
    (0..DAILY_DAYS)
        .map(|back| {
            let (y, m, d) = civil(today - back);
            format!("{y:04}-{m:02}-{d:02}")
        })
        .collect()
}

pub fn files(project: &Project, channels: &[String]) -> Vec<LogFile> {
    let mut names: BTreeSet<String> = [
        "laravel".to_owned(),
        "worker".to_owned(),
        "schedule".to_owned(),
    ]
    .into();
    names.extend(channels.iter().cloned());
    let dates = recent_dates();
    let mut out = Vec::new();
    for name in names {
        let mut candidates = vec![format!("storage/logs/{name}.log")];
        candidates.extend(dates.iter().map(|d| format!("storage/logs/{name}-{d}.log")));
        for path in candidates {
            if let Ok(text) = project.read_file(&path) {
                out.push(LogFile {
                    path,
                    size: text.len(),
                });
            }
        }
    }
    out
}

fn header(line: &str) -> Option<(String, String, String, String)> {
    let rest = line.strip_prefix('[')?;
    let close = rest.find("] ")?;
    let time = &rest[..close];
    if !time.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let after = &rest[close + 2..];
    let colon = after.find(": ")?;
    let (env, level) = after[..colon].split_once('.')?;
    if env.contains(' ') || level.contains(' ') {
        return None;
    }
    Some((
        time.to_owned(),
        env.to_owned(),
        level.to_ascii_lowercase(),
        after[colon + 2..].to_owned(),
    ))
}

pub fn parse(text: &str) -> Vec<LogEntry> {
    let mut out: Vec<LogEntry> = Vec::new();
    for line in text.lines() {
        if let Some((time, env, level, message)) = header(line) {
            let (message, context) = match message.find(" {\"").or_else(|| message.find(" [{")) {
                Some(i) => (message[..i].to_owned(), message[i + 1..].to_owned()),
                None => (message, String::new()),
            };
            out.push(LogEntry {
                time,
                env,
                level,
                message,
                details: context,
            });
        } else if let Some(last) = out.last_mut() {
            if !last.details.is_empty() {
                last.details.push('\n');
            }
            last.details.push_str(line);
        } else if !line.trim().is_empty() {
            out.push(LogEntry {
                time: String::new(),
                env: String::new(),
                level: "info".into(),
                message: line.to_owned(),
                details: String::new(),
            });
        }
    }
    let skip = out.len().saturating_sub(MAX_ENTRIES);
    out.split_off(skip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_with_context_and_traces() {
        let text = "[2026-11-04 10:00:00] local.ERROR: Boom {\"exception\":\"[object] (RuntimeException(code: 0): Boom at /app/X.php:12)\n[stacktrace]\n#0 /app/Y.php(3): f()\n\"} \n[2026-11-04 10:00:01] production.info: Hello\n";
        let entries = parse(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            (
                entries[0].level.as_str(),
                entries[0].env.as_str(),
                entries[0].message.as_str()
            ),
            ("error", "local", "Boom")
        );
        assert!(entries[0].details.contains("#0 /app/Y.php(3)"));
        assert_eq!(entries[1].message, "Hello");
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_396), (2025, 11, 4));
    }
}
