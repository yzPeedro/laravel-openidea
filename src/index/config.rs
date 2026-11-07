use super::Entry;
use crate::php::{array_keys, tokenize};
use crate::project::{Lines, find, rel};
use openidea_plugin::{Location, Project, Range};

pub fn index(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for path in find(project, root, "config/**/*.php", 2000) {
        let relative = rel(root, &path).replace('\\', "/");
        let Some(name) = relative
            .strip_prefix("config/")
            .and_then(|n| n.strip_suffix(".php"))
        else {
            continue;
        };
        let Ok(text) = crate::project::read_listed(project, &path) else {
            continue;
        };
        out.extend(entries(name.replace('/', "."), &relative, &path, &text));
    }
    out
}

pub fn entries(prefix: String, relative: &str, path: &str, text: &str) -> Vec<Entry> {
    let lines = Lines::new(text);
    let mut out = vec![Entry {
        key: prefix.clone(),
        detail: Some(relative.to_owned()),
        location: Location {
            path: path.to_owned(),
            range: Range::default(),
        },
    }];
    out.extend(
        array_keys(&tokenize(text, 0, text.len()))
            .into_iter()
            .map(|k| Entry {
                key: format!("{prefix}.{}", k.path),
                detail: k.value.or_else(|| Some(relative.to_owned())),
                location: Location {
                    path: path.to_owned(),
                    range: lines.range(text, k.start, k.end),
                },
            }),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::Position;

    #[test]
    fn indexes_config_files_with_nested_keys() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file("config/app.php", "<?php\n\nreturn [\n    'name' => env('APP_NAME', 'Laravel'),\n    'providers' => [\n        'cache' => true,\n    ],\n];\n");
        host.add_file(
            "config/services/mail.php",
            "<?php\nreturn ['driver' => 'smtp'];\n",
        );
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let entries = index(&project, "/project");
        let keys: Vec<_> = entries
            .iter()
            .map(|e| (e.key.as_str(), e.detail.as_deref()))
            .collect();
        assert_eq!(
            keys,
            [
                ("app", Some("config/app.php")),
                ("app.name", Some("env('APP_NAME')")),
                ("app.providers", Some("config/app.php")),
                ("app.providers.cache", Some("true")),
                ("services.mail", Some("config/services/mail.php")),
                ("services.mail.driver", Some("'smtp'")),
            ]
        );
        assert_eq!(entries[1].location.path, "/project/config/app.php");
        assert_eq!(entries[1].location.range.start, Position::new(3, 5));
    }
}
