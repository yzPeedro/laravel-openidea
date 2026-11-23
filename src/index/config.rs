use super::Entry;
use crate::php::{array_keys, tokenize};
use crate::project::{Lines, find, rel};
use openidea_plugin::{Location, Project, Range};

pub const FRAMEWORK_DIR: &str = "vendor/laravel/framework/config/";

pub const FRAMEWORK_FILES: [&str; 16] = [
    "app",
    "auth",
    "broadcasting",
    "cache",
    "concurrency",
    "cors",
    "database",
    "filesystems",
    "hashing",
    "images",
    "logging",
    "mail",
    "queue",
    "services",
    "session",
    "view",
];

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
    for name in FRAMEWORK_FILES {
        let relative = format!("{FRAMEWORK_DIR}{name}.php");
        let Ok(text) = project.read_file(&relative) else {
            continue;
        };
        let path = crate::project::join(root, &relative);
        let defaults = entries(name.to_owned(), &relative, &path, &text);
        out.extend(framework_defaults(&out, defaults));
    }
    out
}

pub fn framework_defaults(project: &[Entry], defaults: Vec<Entry>) -> Vec<Entry> {
    let top = |key: &str| -> String {
        let mut parts = key.splitn(3, '.');
        let file = parts.next().unwrap_or_default();
        match parts.next() {
            Some(first) => format!("{file}.{first}"),
            None => file.to_owned(),
        }
    };
    defaults
        .into_iter()
        .filter(|d| {
            let wanted = top(&d.key);
            !project.iter().any(|p| {
                p.key == d.key
                    || (wanted.contains('.')
                        && (p.key == wanted || p.key.starts_with(&format!("{wanted}."))))
            })
        })
        .collect()
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

    #[test]
    fn framework_defaults_fill_what_the_project_leaves_out() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file(
            "config/app.php",
            "<?php\nreturn ['name' => 'Shop', 'providers' => ['a' => 1]];\n",
        );
        host.add_file(
            "vendor/laravel/framework/config/app.php",
            "<?php\nreturn ['name' => 'Laravel', 'env' => 'production', 'providers' => ['b' => 2]];\n",
        );
        host.add_file(
            "vendor/laravel/framework/config/cors.php",
            "<?php\nreturn ['paths' => ['api/*']];\n",
        );
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let keys: Vec<(String, String)> = index(&project, "/project")
            .into_iter()
            .map(|e| {
                let file = e.location.path.trim_start_matches("/project/").to_owned();
                (e.key, file)
            })
            .collect();
        let has = |k: &str, f: &str| keys.contains(&(k.to_owned(), f.to_owned()));
        assert!(has("app.name", "config/app.php"), "{keys:?}");
        assert!(
            has("app.env", "vendor/laravel/framework/config/app.php"),
            "{keys:?}"
        );
        assert!(
            !keys.iter().any(|(k, _)| k == "app.providers.b"),
            "the project's array wins: {keys:?}"
        );
        assert!(
            !keys
                .iter()
                .any(|(k, f)| k == "app" && f.starts_with("vendor/")),
            "{keys:?}"
        );
        assert!(
            has("cors", "vendor/laravel/framework/config/cors.php"),
            "{keys:?}"
        );
        assert!(
            has("cors.paths", "vendor/laravel/framework/config/cors.php"),
            "{keys:?}"
        );
    }
}
