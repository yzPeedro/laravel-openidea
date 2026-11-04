use super::Entry;
use crate::project::{Lines, join};
use openidea_plugin::{Location, Project};
use std::collections::BTreeSet;

pub fn index(project: &Project, root: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for name in [".env", ".env.example"] {
        let Ok(text) = project.read_file(name) else {
            continue;
        };
        let path = join(root, name);
        for entry in parse(&path, &text, name == ".env.example") {
            if seen.insert(entry.key.clone()) {
                out.push(entry);
            }
        }
    }
    out
}

pub fn parse(path: &str, text: &str, show_values: bool) -> Vec<Entry> {
    let lines = Lines::new(text);
    let mut out = Vec::new();
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let skip = line.len() - indent - trimmed.len();
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let key = trimmed[..eq].trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            continue;
        }
        let key_start = start + indent + skip;
        let value = trimmed[eq + 1..].trim();
        out.push(Entry {
            key: key.to_owned(),
            detail: if show_values && !value.is_empty() {
                Some(value.to_owned())
            } else {
                None
            },
            location: Location {
                path: path.to_owned(),
                range: lines.range(text, key_start, key_start + key.len()),
            },
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_keys_without_leaking_secret_values() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file(
            ".env",
            "APP_NAME=Shop\n# comment\nAPP_KEY=base64:secret\n\nexport DB_HOST=127.0.0.1\n",
        );
        host.add_file(".env.example", "APP_NAME=Laravel\nMAIL_MAILER=log\n");
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let entries = index(&project, "/project");
        let keys: Vec<_> = entries
            .iter()
            .map(|e| (e.key.as_str(), e.detail.as_deref()))
            .collect();
        assert_eq!(
            keys,
            [
                ("APP_NAME", None),
                ("APP_KEY", None),
                ("DB_HOST", None),
                ("MAIL_MAILER", Some("log"))
            ]
        );
        assert_eq!(entries[2].location.range.start.character, 7);
    }
}
