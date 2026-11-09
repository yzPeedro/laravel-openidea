use crate::Laravel;
use crate::audit;
use crate::devtools;
use crate::index::Entry;
use crate::php;
use crate::project::{abs, find, rel};
use crate::refs;
use crate::runner;
use openidea_plugin::{Project, Range, host};
use std::collections::{BTreeMap, BTreeSet};

pub const STUB_PATH: &str = "storage/openidea/ide-helpers.php";

const SKIPPED: [&str; 4] = ["vendor/", "node_modules/", ".git/", "storage/framework/"];

pub fn known_dirs(base: &str) -> &'static [&'static str] {
    match base {
        "storage/" => &[
            "app",
            "app/private",
            "app/public",
            "framework",
            "framework/cache",
            "framework/sessions",
            "framework/views",
            "logs",
        ],
        "" => &[
            "app",
            "bootstrap",
            "config",
            "database",
            "lang",
            "public",
            "resources",
            "routes",
            "storage",
            "tests",
        ],
        _ => &[],
    }
}

pub fn path_entries(files: &[String], base: &str) -> Vec<Entry> {
    let mut seen_files = BTreeSet::new();
    let mut dirs = BTreeMap::new();
    let mut out = Vec::new();
    for (path, absolute) in files.iter().filter_map(|f| f.split_once('\u{0}')) {
        if base.is_empty() && SKIPPED.iter().any(|s| path.starts_with(s)) {
            continue;
        }
        let Some(inner) = path.strip_prefix(base) else {
            continue;
        };
        if inner.is_empty() || !seen_files.insert(inner.to_owned()) {
            continue;
        }
        let prefix = absolute.strip_suffix(inner).unwrap_or("");
        let folder = inner.ends_with('/');
        let mut acc = String::new();
        let parts: Vec<&str> = inner.trim_end_matches('/').split('/').collect();
        let dirs_in = if folder { parts.len() } else { parts.len() - 1 };
        for part in &parts[..dirs_in] {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(part);
            dirs.entry(acc.clone())
                .or_insert_with(|| format!("{prefix}{acc}"));
        }
        if folder {
            continue;
        }
        out.push(Entry::new(
            inner,
            Some("file".into()),
            absolute,
            Range::default(),
        ));
    }
    out.extend(
        dirs.into_iter()
            .map(|(dir, folder)| Entry::new(dir, Some("folder".into()), &folder, Range::default())),
    );
    out
}

pub fn list_files(project: &Project, root: &str, base: &str) -> Vec<String> {
    let globs: Vec<String> = if base.is_empty() {
        vec!["*".into(), "*/*".into(), "*/*/*".into()]
    } else {
        vec![format!("{base}**/*")]
    };
    let mut out = Vec::new();
    for glob in globs {
        for path in crate::project::find_some(project, root, &glob, 3000) {
            let relative = rel(root, &path).replace('\\', "/");
            out.push(format!("{relative}\u{0}{path}"));
        }
    }
    out
}

fn quoted(text: &str) -> Option<&str> {
    let start = text.find(['\'', '"'])?;
    let quote = &text[start..=start];
    let rest = &text[start + 1..];
    Some(&rest[..rest.find(quote)?])
}

pub fn default_disk(config: &str) -> String {
    config
        .find("'default'")
        .and_then(|i| {
            let line = config[i..].lines().next()?;
            let value = line.split_once("=>")?.1;
            let last = value.rfind(['\'', '"'])?;
            let before = &value[..last];
            let open = before.rfind(['\'', '"'])?;
            Some(value[open + 1..last].to_owned())
        })
        .filter(|d| !d.is_empty() && !d.contains(' '))
        .unwrap_or_else(|| "local".into())
}

fn block(text: &str, open: usize) -> Option<&str> {
    let mut depth = 0usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn disk_root(config: &str, disk: Option<&str>) -> Option<String> {
    let name = disk.map_or_else(|| default_disk(config), str::to_owned);
    let fallback = match name.as_str() {
        "local" => Some("storage/app/private/".to_owned()),
        "public" => Some("storage/app/public/".to_owned()),
        _ => None,
    };
    let Some(disks) = config.find("'disks'") else {
        return fallback;
    };
    let after = &config[disks..];
    let Some(at) = [format!("'{name}'"), format!("\"{name}\"")]
        .iter()
        .find_map(|k| after.find(&format!("{k} =>")))
    else {
        return fallback;
    };
    let open = at + after[at..].find('[')?;
    let body = block(after, open)?;
    if let Some(i) = body.find("'driver'")
        && quoted(body[i + 8..].split_once("=>")?.1)? != "local"
    {
        return None;
    }
    let Some(i) = body.find("'root'") else {
        return fallback;
    };
    let expr = body[i + 6..].split_once("=>")?.1.trim_start();
    let inner = quoted(expr.split([',', '\n']).next().unwrap_or(expr)).unwrap_or("");
    let prefix = [
        "storage_path",
        "public_path",
        "base_path",
        "app_path",
        "resource_path",
    ]
    .into_iter()
    .find(|f| expr.starts_with(f))
    .map(|f| refs::path_base(f).unwrap_or(""));
    let path = match prefix {
        Some(base) => format!("{base}{inner}"),
        None if expr.starts_with(['\'', '"']) => inner.to_owned(),
        None => return fallback,
    };
    let path = path.trim_matches('/');
    Some(if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    })
}

pub fn disk_before(text: &str, call_start: usize) -> Option<String> {
    let before = text.get(..call_start)?.trim_end();
    let before = before
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
        .trim_end();
    let before = before.strip_suffix("->")?.trim_end().strip_suffix(')')?;
    let open = before.rfind('(')?;
    let name = before[..open].trim_end();
    if !name.to_ascii_lowercase().ends_with("disk") {
        return None;
    }
    quoted(&before[open + 1..]).map(str::to_owned)
}

pub fn rule_fields(text: &str, blade: bool) -> Vec<String> {
    let mut out = BTreeSet::new();
    for arg in php::args_in(text, blade) {
        if refs::is_rule_key(&arg) && !arg.value.is_empty() && !arg.value.contains(['*', '$', '{'])
        {
            out.insert(arg.value);
        }
    }
    out.into_iter().collect()
}

pub fn typed_requests(text: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(found) = text[i..].find("Request") {
        let end = i + found + "Request".len();
        let mut start = i + found;
        while start > 0
            && (bytes[start - 1].is_ascii_alphanumeric()
                || bytes[start - 1] == b'_'
                || bytes[start - 1] == b'\\')
        {
            start -= 1;
        }
        let name = &text[start..end];
        let next = text[end..].trim_start();
        if next.starts_with('$') && name.rsplit('\\').next() != Some("Request") && !name.is_empty()
        {
            out.insert(name.to_owned());
        }
        i = end;
    }
    out.into_iter().collect()
}

pub fn user_model(auth_config: &str) -> String {
    let from = auth_config
        .find("'providers'")
        .and_then(|p| auth_config[p..].find("'model'").map(|m| p + m));
    from.and_then(|at| {
        let line = auth_config[at..].lines().next()?;
        let value = line.split_once("=>")?.1;
        let class = value.rfind("::class").map(|end| {
            let before = &value[..end];
            let start = before
                .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'))
                .map_or(0, |s| s + 1);
            before[start..].to_owned()
        });
        class.or_else(|| quoted(value).map(str::to_owned))
    })
    .map(|c| c.trim_start_matches('\\').to_owned())
    .filter(|c| !c.is_empty())
    .unwrap_or_else(|| "App\\Models\\User".into())
}

pub fn stub(user_model: &str) -> String {
    format!(
        "<?php\n\nnamespace OpenIdea\\Stubs {{\n    /**\n     * @method \\{user_model}|null user()\n     */\n    class AuthManager extends \\Illuminate\\Auth\\AuthManager\n    {{\n    }}\n}}\n\nnamespace {{\n    /**\n     * @return ($guard is null ? \\OpenIdea\\Stubs\\AuthManager|\\Illuminate\\Contracts\\Auth\\StatefulGuard : \\Illuminate\\Contracts\\Auth\\StatefulGuard)\n     */\n    function auth($guard = null)\n    {{\n    }}\n}}\n"
    )
}

impl Laravel {
    pub(crate) fn sync_helpers(&mut self, project: &Project) {
        let Ok(root) = project.root() else {
            return;
        };
        let enabled = host::setting_bool("ideHelpers").unwrap_or(true);
        let current = project.read_file(STUB_PATH).ok();
        let wanted = if enabled {
            stub(&user_model(
                &project.read_file("config/auth.php").unwrap_or_default(),
            ))
        } else {
            String::new()
        };
        if current.as_deref().unwrap_or("") == wanted {
            return;
        }
        if self.helpers_synced.get(&root) == Some(&wanted) {
            return;
        }
        self.helpers_synced.insert(root.clone(), wanted.clone());
        let mode = runner::detect(project);
        let _task = host::begin_task("Laravel: updating the IDE helper stubs", false);
        let run = mode.php_args(&[
            "-r".into(),
            devtools::script(devtools::INSTALL_PHP),
            "helpers".into(),
        ]);
        match crate::runner::run(project, &mode, &run, Some(&wanted)) {
            Ok(out) if out.status != Some(0) => {
                audit::error("IDE helper sync", audit::output(&out));
            }
            Ok(_) => {
                if project.read_file(STUB_PATH).unwrap_or_default() != wanted {
                    audit::error(
                        "IDE helper sync",
                        format!("{STUB_PATH} is still out of date after the sync"),
                    );
                }
            }
            Err(e) => audit::failed("IDE helper sync", &e),
        }
    }

    pub(crate) fn path_items(&mut self, project: &Project, root: &str, base: &str) -> Vec<Entry> {
        let base = if base == "lang/"
            && crate::project::find(project, root, "lang/*", 1).is_empty()
            && !crate::project::find(project, root, "resources/lang/*", 1).is_empty()
        {
            "resources/lang/"
        } else {
            base
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let key = format!("{root}\u{0}{base}");
        let fresh = self
            .listed
            .get(&key)
            .is_some_and(|(at, _)| now.saturating_sub(*at) < 15);
        if !fresh {
            let mut files = list_files(project, root, base);
            for dir in known_dirs(base) {
                files.push(format!(
                    "{base}{dir}/\u{0}{}",
                    abs(root, &format!("{base}{dir}/"))
                ));
            }
            self.listed.insert(key.clone(), (now, files));
        }
        path_entries(&self.listed[&key].1, base)
    }

    pub(crate) fn request_fields(
        &mut self,
        project: &Project,
        root: &str,
        text: &str,
        blade: bool,
    ) -> Vec<Entry> {
        let mut fields: BTreeMap<String, String> = BTreeMap::new();
        for f in rule_fields(text, blade) {
            fields
                .entry(f)
                .or_insert_with(|| "validated in this file".into());
        }
        let composer = project.read_file("composer.json").unwrap_or_default();
        let map = crate::casts::psr4(&composer);
        let mut classes: Vec<(String, String)> = typed_requests(text)
            .into_iter()
            .filter_map(|name| {
                let fqn = crate::casts::resolve_name(text, &name);
                crate::casts::file_of(project, &map, &fqn).map(|(_, t)| (name, t))
            })
            .collect();
        if blade {
            for path in find(project, root, "app/Http/Requests/**/*.php", 300) {
                let relative = rel(root, &path).to_owned();
                if let Ok(t) = project.read_file(&relative) {
                    let name = relative
                        .rsplit('/')
                        .next()
                        .unwrap_or(&relative)
                        .trim_end_matches(".php")
                        .to_owned();
                    classes.push((name, t));
                }
            }
        }
        for (name, t) in classes {
            let short = name.rsplit('\\').next().unwrap_or(&name).to_owned();
            for f in rule_fields(&t, false) {
                fields.entry(f).or_insert_with(|| short.clone());
            }
        }
        let file = abs(root, "");
        fields
            .into_iter()
            .map(|(k, d)| Entry::new(k, Some(d), &file, Range::default()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| format!("{p}\u{0}/p/{p}")).collect()
    }

    fn keys(entries: &[Entry]) -> Vec<(String, String)> {
        let mut out: Vec<_> = entries
            .iter()
            .map(|e| (e.key.clone(), e.detail.clone().unwrap_or_default()))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn lists_files_and_folders_under_a_base() {
        let files = listed(&[
            "public/css/app.css",
            "public/favicon.ico",
            "public/img/logo/mark.svg",
            "app/User.php",
        ]);
        assert_eq!(
            keys(&path_entries(&files, "public/")),
            [
                ("css".into(), "folder".into()),
                ("css/app.css".into(), "file".into()),
                ("favicon.ico".into(), "file".into()),
                ("img".into(), "folder".into()),
                ("img/logo".into(), "folder".into()),
                ("img/logo/mark.svg".into(), "file".into()),
            ]
        );
        let storage = listed(&["storage/app/public/a.png", "storage/logs/"]);
        assert_eq!(
            keys(&path_entries(&storage, "storage/")),
            [
                ("app".into(), "folder".into()),
                ("app/public".into(), "folder".into()),
                ("app/public/a.png".into(), "file".into()),
                ("logs".into(), "folder".into()),
            ]
        );
        let root = listed(&[
            "composer.json",
            "vendor/autoload.php",
            "app/Models/User.php",
            "storage/framework/x",
        ]);
        let found = keys(&path_entries(&root, ""));
        assert!(found.contains(&("composer.json".into(), "file".into())));
        assert!(found.contains(&("app/Models".into(), "folder".into())));
        assert!(
            !found
                .iter()
                .any(|(k, _)| k.starts_with("vendor") || k.starts_with("storage/framework"))
        );
    }

    const FILESYSTEMS: &str = "<?php\nreturn [\n    'default' => env('FILESYSTEM_DISK', 'local'),\n    'disks' => [\n        'local' => [\n            'driver' => 'local',\n            'root' => storage_path('app/private'),\n            'serve' => true,\n        ],\n        'public' => [\n            'driver' => 'local',\n            'root' => storage_path('app/public'),\n            'url' => env('APP_URL').'/storage',\n        ],\n        'exports' => [\n            'driver' => 'local',\n            'root' => base_path('exports'),\n        ],\n        's3' => [\n            'driver' => 's3',\n            'key' => env('AWS_ACCESS_KEY_ID'),\n        ],\n    ],\n];\n";

    #[test]
    fn finds_the_root_of_each_local_disk() {
        assert_eq!(default_disk(FILESYSTEMS), "local");
        assert_eq!(
            disk_root(FILESYSTEMS, None).as_deref(),
            Some("storage/app/private/")
        );
        assert_eq!(
            disk_root(FILESYSTEMS, Some("public")).as_deref(),
            Some("storage/app/public/")
        );
        assert_eq!(
            disk_root(FILESYSTEMS, Some("exports")).as_deref(),
            Some("exports/")
        );
        assert_eq!(disk_root(FILESYSTEMS, Some("s3")), None);
        assert_eq!(disk_root("", None).as_deref(), Some("storage/app/private/"));
    }

    #[test]
    fn reads_the_disk_of_a_chained_call() {
        let text = "Storage::disk('public')->exists('";
        let call = text.rfind('(').unwrap();
        assert_eq!(disk_before(text, call).as_deref(), Some("public"));
        let text = "Storage::exists('";
        assert_eq!(disk_before(text, text.rfind('(').unwrap()), None);
    }

    #[test]
    fn collects_validated_fields_and_typed_form_requests() {
        let text = "<?php\nclass OrderController {\n    public function store(StoreOrderRequest $request, Request $plain) {\n        $request->validate(['email' => 'required|email', 'items.*.id' => 'int', 'note' => ['nullable']]);\n    }\n}\n";
        assert_eq!(rule_fields(text, false), ["email", "note"]);
        assert_eq!(typed_requests(text), ["StoreOrderRequest"]);
        let request = "<?php\nclass StoreOrderRequest extends FormRequest {\n    public function rules(): array {\n        return ['coupon' => 'nullable|string', 'total' => 'numeric'];\n    }\n}\n";
        assert_eq!(rule_fields(request, false), ["coupon", "total"]);
    }

    #[test]
    fn types_auth_with_the_configured_user_model() {
        let config = "<?php\nreturn [\n    'providers' => [\n        'users' => [\n            'driver' => 'eloquent',\n            'model' => env('AUTH_MODEL', App\\Models\\Customer::class),\n        ],\n    ],\n];\n";
        assert_eq!(user_model(config), "App\\Models\\Customer");
        assert_eq!(user_model(""), "App\\Models\\User");
        let stub = stub("App\\Models\\Customer");
        assert!(stub.contains("@method \\App\\Models\\Customer|null user()"));
        assert!(stub.contains("function auth($guard = null)"));
        assert!(stub.starts_with("<?php\n"));
    }
}
