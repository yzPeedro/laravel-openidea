use crate::toolconfig::{Completion, Suggestion, json_context, key, value};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: String,
}

pub fn packages(lock: &str) -> Vec<Package> {
    let Ok(json) = serde_json::from_str::<Value>(lock) else {
        return Vec::new();
    };
    ["packages", "packages-dev"]
        .iter()
        .filter_map(|k| json.get(*k)?.as_array())
        .flatten()
        .filter_map(|p| {
            Some(Package {
                name: p.get("name")?.as_str()?.to_owned(),
                version: p
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim_start_matches('v')
                    .to_owned(),
                description: p
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                kind: p
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("library")
                    .to_owned(),
            })
        })
        .collect()
}

const ROOT_KEYS: &[(&str, &str)] = &[
    ("name", "vendor/package"),
    ("description", "one line about the package"),
    ("type", "project, library, metapackage or composer-plugin"),
    ("keywords", "search keywords"),
    ("license", "SPDX license"),
    ("homepage", "project URL"),
    ("authors", "people behind the package"),
    ("support", "issues, docs, source…"),
    ("funding", "ways to fund the package"),
    ("require", "packages needed to run"),
    ("require-dev", "packages needed to develop and test"),
    ("conflict", "packages that can't be installed alongside"),
    ("replace", "packages this one replaces"),
    ("provide", "virtual packages this one provides"),
    ("suggest", "optional packages"),
    ("autoload", "PSR-4, classmap and files for the app"),
    ("autoload-dev", "PSR-4, classmap and files for tests"),
    ("repositories", "extra package sources"),
    ("config", "Composer settings for the project"),
    ("scripts", "commands run on events or with composer run"),
    ("scripts-descriptions", "help text for custom scripts"),
    ("scripts-aliases", "other names for scripts"),
    ("extra", "data for plugins and Laravel package discovery"),
    ("bin", "files to link into vendor/bin"),
    ("minimum-stability", "lowest stability allowed"),
    ("prefer-stable", "prefer stable versions when possible"),
    ("version", "package version (usually from VCS tags)"),
];

const KNOWN_PACKAGES: &[(&str, &str)] = &[
    ("php", "PHP version"),
    ("laravel/framework", "The Laravel framework"),
    ("laravel/tinker", "Interactive REPL for Laravel"),
    ("laravel/sanctum", "API tokens and SPA authentication"),
    ("laravel/passport", "OAuth2 server"),
    (
        "laravel/fortify",
        "Frontend-agnostic authentication backend",
    ),
    ("laravel/socialite", "OAuth login with social providers"),
    (
        "laravel/horizon",
        "Dashboard and supervisor for Redis queues",
    ),
    ("laravel/telescope", "Debug assistant"),
    ("laravel/pulse", "Production performance dashboard"),
    ("laravel/reverb", "WebSocket server"),
    ("laravel/octane", "High-performance application server"),
    ("laravel/scout", "Full-text search for Eloquent"),
    ("laravel/cashier", "Stripe subscription billing"),
    ("laravel/pennant", "Feature flags"),
    ("laravel/folio", "Page-based routing"),
    ("laravel/prompts", "Beautiful CLI forms"),
    ("laravel/pail", "Tail application logs"),
    ("laravel/pint", "Code style fixer"),
    ("laravel/sail", "Docker development environment"),
    ("laravel/boost", "AI agent guidelines and MCP tools"),
    (
        "laravel/wayfinder",
        "TypeScript functions for routes and actions",
    ),
    ("laravel/nightwatch", "Laravel monitoring agent"),
    ("livewire/livewire", "Full-stack components"),
    ("livewire/volt", "Single-file Livewire components"),
    ("inertiajs/inertia-laravel", "Inertia.js server adapter"),
    ("tightenco/ziggy", "Laravel routes in JavaScript"),
    ("filament/filament", "Admin panels and forms"),
    ("spatie/laravel-permission", "Roles and permissions"),
    ("spatie/laravel-medialibrary", "Files attached to models"),
    ("spatie/laravel-data", "Typed data objects"),
    (
        "spatie/laravel-query-builder",
        "Filter, sort and include from requests",
    ),
    ("spatie/laravel-backup", "Backups of files and databases"),
    ("spatie/laravel-activitylog", "Activity log of models"),
    ("spatie/laravel-ray", "Debug with Ray"),
    ("barryvdh/laravel-debugbar", "Debug bar"),
    ("barryvdh/laravel-ide-helper", "IDE helper files"),
    ("barryvdh/laravel-dompdf", "PDF generation"),
    ("maatwebsite/excel", "Excel imports and exports"),
    ("itsgoingd/clockwork", "Development profiler"),
    ("larastan/larastan", "PHPStan for Laravel"),
    ("pestphp/pest", "Testing framework"),
    ("pestphp/pest-plugin-laravel", "Pest helpers for Laravel"),
    ("phpunit/phpunit", "Testing framework"),
    ("mockery/mockery", "Mock objects"),
    ("fakerphp/faker", "Fake data"),
    ("nunomaduro/collision", "Error reporting for the console"),
    ("guzzlehttp/guzzle", "HTTP client"),
    ("predis/predis", "Redis client in PHP"),
    ("league/flysystem-aws-s3-v3", "S3 driver for Storage"),
    ("sentry/sentry-laravel", "Error tracking"),
    ("rector/rector", "Automated refactoring"),
    ("friendsofphp/php-cs-fixer", "Code style fixer"),
];

const PHP_EXTENSIONS: &[&str] = &[
    "ext-bcmath",
    "ext-ctype",
    "ext-curl",
    "ext-dom",
    "ext-fileinfo",
    "ext-gd",
    "ext-intl",
    "ext-json",
    "ext-mbstring",
    "ext-openssl",
    "ext-pcntl",
    "ext-pdo",
    "ext-pdo_mysql",
    "ext-pdo_pgsql",
    "ext-pdo_sqlite",
    "ext-redis",
    "ext-sodium",
    "ext-tokenizer",
    "ext-xml",
    "ext-zip",
];

const SCRIPT_EVENTS: &[(&str, &str)] = &[
    ("post-autoload-dump", "after the autoloader is dumped"),
    ("pre-autoload-dump", "before the autoloader is dumped"),
    ("post-install-cmd", "after composer install"),
    ("pre-install-cmd", "before composer install"),
    ("post-update-cmd", "after composer update"),
    ("pre-update-cmd", "before composer update"),
    (
        "post-root-package-install",
        "after create-project installs the root package",
    ),
    ("post-create-project-cmd", "after create-project"),
    ("pre-package-uninstall", "before a package is removed"),
    ("post-package-install", "after a package is installed"),
    ("post-package-update", "after a package is updated"),
    ("dev", "custom: composer run dev"),
    ("test", "custom: composer test"),
    ("lint", "custom: composer lint"),
    ("analyse", "custom: composer analyse"),
    ("format", "custom: composer format"),
    ("setup", "custom: composer setup"),
];

const SCRIPT_COMMANDS: &[(&str, &str)] = &[
    (
        "Illuminate\\\\Foundation\\\\ComposerScripts::postAutoloadDump",
        "Laravel's autoload hook",
    ),
    (
        "@php artisan package:discover --ansi",
        "discover package providers",
    ),
    (
        "@php artisan vendor:publish --tag=laravel-assets --ansi --force",
        "publish framework assets",
    ),
    ("@php artisan key:generate --ansi", "generate APP_KEY"),
    ("@php artisan migrate --graceful --ansi", "run migrations"),
    ("@php artisan config:clear --ansi", "clear the config cache"),
    ("@php artisan optimize:clear --ansi", "clear every cache"),
    ("@php artisan test", "run the tests"),
    (
        "@php artisan boost:update --ansi",
        "refresh Laravel Boost guidelines",
    ),
    ("@php artisan ide-helper:generate", "IDE helper file"),
    ("@php vendor/bin/pint", "fix code style"),
    ("@php vendor/bin/phpstan analyse", "static analysis"),
    ("@php vendor/bin/pest", "run Pest"),
    (
        "@php -r \\\"file_exists('.env') || copy('.env.example', '.env');\\\"",
        "create .env from .env.example",
    ),
    (
        "@putenv COMPOSER=composer.json",
        "set an environment variable",
    ),
    ("npm install", "install front-end packages"),
    ("npm run build", "build front-end assets"),
];

const CONFIG_KEYS: &[(&str, &str)] = &[
    ("optimize-autoloader", "class map for every PSR-4 class"),
    (
        "classmap-authoritative",
        "load classes only from the class map",
    ),
    ("preferred-install", "dist, source or auto"),
    ("sort-packages", "keep require sorted"),
    ("allow-plugins", "plugins allowed to run"),
    ("platform", "fake platform packages (php, ext-*)"),
    ("platform-check", "check the platform on autoload"),
    ("process-timeout", "seconds before a script is stopped"),
    ("vendor-dir", "where packages are installed"),
    ("bin-dir", "where binaries are linked"),
    ("discard-changes", "what to do with local changes in vendor"),
    ("audit", "security audit settings"),
    ("lock", "write composer.lock"),
    ("notify-on-install", "notify the repository on install"),
];

fn suggestions(list: &[(&str, &str)], as_value: bool) -> Vec<Suggestion> {
    list.iter()
        .map(|(l, d)| if as_value { value(l, d) } else { key(l, d) })
        .collect()
}

fn package_names(installed: &[Package], as_value: bool) -> Vec<Suggestion> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for p in installed {
        if seen.insert(p.name.clone()) {
            let detail = format!("{} · installed {}", p.description, p.version);
            out.push(if as_value {
                value(&p.name, &detail)
            } else {
                key(&p.name, &detail)
            });
        }
    }
    for (name, detail) in KNOWN_PACKAGES {
        if seen.insert((*name).to_owned()) {
            out.push(if as_value {
                value(name, detail)
            } else {
                key(name, detail)
            });
        }
    }
    for ext in PHP_EXTENSIONS {
        if seen.insert((*ext).to_owned()) {
            out.push(key(ext, "PHP extension"));
        }
    }
    out
}

fn versions(package: &str, installed: &[Package]) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if let Some(p) = installed.iter().find(|p| p.name == package)
        && !p.version.is_empty()
    {
        let parts: Vec<&str> = p.version.split('.').collect();
        if let [major, minor, ..] = parts.as_slice() {
            out.push(value(
                &format!("^{major}.{minor}"),
                "installed minor and up",
            ));
            out.push(value(&format!("^{major}.0"), "installed major"));
        }
        out.push(value(&p.version, "exactly the installed version"));
    }
    if package == "php" {
        for v in ["^8.2", "^8.3", "^8.4", "^8.5"] {
            out.push(value(v, "PHP version"));
        }
    }
    out.push(value("*", "any version"));
    out.push(value("dev-main", "the main branch"));
    out
}

pub fn composer(text: &str, cursor: usize, installed: &[Package]) -> Option<Completion> {
    let (start, end, path, role) = json_context(text, cursor)?;
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    let items = match (path.as_slice(), role.as_deref()) {
        ([], None) => suggestions(ROOT_KEYS, false),
        ([], Some("minimum-stability")) => ["stable", "RC", "beta", "alpha", "dev"]
            .iter()
            .map(|s| value(s, "stability"))
            .collect(),
        ([], Some("type")) => ["project", "library", "metapackage", "composer-plugin"]
            .iter()
            .map(|s| value(s, "package type"))
            .collect(),
        ([], Some("license")) => [
            "MIT",
            "proprietary",
            "Apache-2.0",
            "BSD-3-Clause",
            "GPL-3.0-or-later",
            "LGPL-3.0-or-later",
        ]
        .iter()
        .map(|s| value(s, "SPDX license"))
        .collect(),
        (["require" | "require-dev" | "conflict" | "replace" | "provide" | "suggest"], None) => {
            package_names(installed, false)
        }
        (["require" | "require-dev" | "conflict"], Some(package)) if !package.is_empty() => {
            versions(package, installed)
        }
        (["autoload" | "autoload-dev"], None) => suggestions(
            &[
                ("psr-4", "namespace → folder"),
                ("classmap", "folders scanned for classes"),
                ("files", "files always loaded (helpers)"),
                ("exclude-from-classmap", "paths left out of the class map"),
                ("psr-0", "legacy namespace mapping"),
            ],
            false,
        ),
        (["autoload" | "autoload-dev", "psr-4"], None) => suggestions(
            &[
                ("App\\\\", "app/"),
                ("Database\\\\Factories\\\\", "database/factories/"),
                ("Database\\\\Seeders\\\\", "database/seeders/"),
                ("Tests\\\\", "tests/"),
            ],
            false,
        ),
        (["autoload" | "autoload-dev", "psr-4"], Some(_))
        | (
            [
                "autoload" | "autoload-dev",
                "classmap" | "files" | "exclude-from-classmap",
            ],
            Some(_),
        ) => suggestions(
            &[
                ("app/", "application classes"),
                ("database/factories/", "model factories"),
                ("database/seeders/", "seeders"),
                ("tests/", "tests"),
                ("src/", "package sources"),
                ("app/helpers.php", "helper functions"),
            ],
            true,
        ),
        (["scripts"], None) | (["scripts-descriptions"], None) => suggestions(SCRIPT_EVENTS, false),
        (["scripts"], Some(_)) | (["scripts", _], Some(_)) => suggestions(SCRIPT_COMMANDS, true),
        (["config"], None) => suggestions(CONFIG_KEYS, false),
        (["config"], Some("preferred-install")) => suggestions(
            &[
                ("dist", "zip archives"),
                ("source", "git clones"),
                ("auto", "source for dev versions"),
            ],
            true,
        ),
        (["config"], Some("discard-changes")) => suggestions(&[("stash", ""), ("true", "")], true),
        (["config", "allow-plugins"], None) => installed
            .iter()
            .filter(|p| p.kind == "composer-plugin")
            .map(|p| key(&p.name, &p.description))
            .chain(
                ["pestphp/pest-plugin", "php-http/discovery"]
                    .iter()
                    .map(|n| key(n, "Composer plugin")),
            )
            .collect(),
        (["config", "platform"], None) => std::iter::once(key("php", "PHP version"))
            .chain(PHP_EXTENSIONS.iter().map(|e| key(e, "PHP extension")))
            .collect(),
        (["extra"], None) => suggestions(
            &[
                ("laravel", "package discovery for Laravel"),
                ("branch-alias", "version of a dev branch"),
            ],
            false,
        ),
        (["extra", "laravel"], None) => suggestions(
            &[
                (
                    "dont-discover",
                    "packages whose providers aren't auto-registered",
                ),
                ("providers", "service providers to register"),
                ("aliases", "facade aliases to register"),
            ],
            false,
        ),
        (["extra", "laravel", "dont-discover"], Some(_)) => {
            let mut list = vec![value("*", "every package")];
            list.extend(
                package_names(installed, true)
                    .into_iter()
                    .filter(|s| s.label.contains('/')),
            );
            list
        }
        _ => return None,
    };
    Some(Completion { start, end, items })
}

#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub attributes: Vec<(String, String)>,
    pub elements: Vec<String>,
    pub orders: Vec<String>,
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let at = text.find(open)? + open.len();
    let end = text[at..].find(close)? + at;
    Some(&text[at..end])
}

fn attr_values(text: &str, name: &str) -> Vec<String> {
    let needle = format!("{name}=\"");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        rest = &rest[i + needle.len()..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_owned());
        }
    }
    out
}

pub fn phpunit_schema(xsd: &str) -> Schema {
    let mut schema = Schema::default();
    if let Some(group) = between(
        xsd,
        "<xs:attributeGroup name=\"configAttributeGroup\">",
        "</xs:attributeGroup>",
    ) {
        for chunk in group.split("<xs:attribute ").skip(1) {
            let name = attr_values(chunk, "name").into_iter().next();
            let kind = attr_values(chunk, "type")
                .into_iter()
                .next()
                .unwrap_or_default();
            if let Some(name) = name {
                let kind = kind.rsplit(':').next().unwrap_or(&kind).to_owned();
                schema.attributes.push((name, kind));
            }
        }
    }
    let elements: std::collections::BTreeSet<String> = xsd
        .split("<xs:element ")
        .skip(1)
        .filter_map(|c| attr_values(c, "name").into_iter().next())
        .collect();
    schema.elements = elements.into_iter().collect();
    if let Some(order) = between(
        xsd,
        "<xs:simpleType name=\"executionOrderType\">",
        "</xs:simpleType>",
    ) {
        schema.orders = attr_values(order, "value");
    }
    schema
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlWant {
    Element,
    Attribute {
        tag: String,
    },
    Value {
        tag: String,
        attr: String,
        name: Option<String>,
    },
    Text {
        tag: String,
    },
}

fn is_name(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
}

pub fn xml_context(text: &str, cursor: usize) -> Option<(usize, usize, XmlWant)> {
    let cursor = cursor.min(text.len());
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let before = &text[..cursor];
    let lt = before.rfind('<');
    let gt = before.rfind('>');
    match (lt, gt) {
        (Some(open), gt) if gt.is_none_or(|g| g < open) => {
            let tag_text = &text[open + 1..];
            let tag_end = tag_text.find('>').map_or(text.len(), |i| open + 1 + i);
            let inside = &text[open + 1..cursor];
            if inside.starts_with(['/', '!', '?']) {
                return None;
            }
            let name_len = inside.find(|c: char| !is_name(c)).unwrap_or(inside.len());
            if name_len == inside.len() {
                let end = text[cursor..]
                    .find(|c: char| !is_name(c))
                    .map_or(text.len(), |i| cursor + i);
                return Some((open + 1, end, XmlWant::Element));
            }
            let tag = inside[..name_len].to_owned();
            let quotes = inside.matches('"').count();
            if quotes % 2 == 1 {
                let q = before.rfind('"')?;
                let attr_end = text[..q].trim_end().strip_suffix('=')?.trim_end();
                let attr_start = attr_end.rfind(|c: char| !is_name(c)).map_or(0, |i| i + 1);
                let attr = attr_end[attr_start..].to_owned();
                let end = text[cursor..].find('"').map_or(cursor, |i| cursor + i);
                let whole = &text[open..tag_end.min(text.len())];
                let name = attr_values(whole, "name").into_iter().next();
                return Some((q + 1, end, XmlWant::Value { tag, attr, name }));
            }
            let start = before
                .rfind(|c: char| !is_name(c))
                .map_or(0, |i| i + 1)
                .max(open + 1 + name_len);
            let end = text[cursor..]
                .find(|c: char| !is_name(c))
                .map_or(text.len(), |i| cursor + i);
            Some((start, end, XmlWant::Attribute { tag }))
        }
        (_, Some(close)) => {
            let opened = &text[..close];
            let tag_open = opened.rfind('<')?;
            let tag_text = &text[tag_open + 1..close];
            if tag_text.starts_with(['/', '!', '?']) || tag_text.ends_with('/') {
                return None;
            }
            let tag: String = tag_text.chars().take_while(|c| is_name(*c)).collect();
            let content = &text[close + 1..cursor];
            if content.contains('<') {
                return None;
            }
            let lead = content.len() - content.trim_start().len();
            let start = close + 1 + lead;
            let end = text[cursor..].find('<').map_or(text.len(), |i| cursor + i);
            let end = start + text[start..end].trim_end().len();
            Some((start, end.max(cursor), XmlWant::Text { tag }))
        }
        _ => None,
    }
}

const PHPUNIT_ELEMENT_HINTS: &[(&str, &str)] = &[
    ("testsuites", "groups of tests"),
    ("testsuite", "one suite, with name=\"…\""),
    ("directory", "a folder of tests or sources"),
    ("file", "one file"),
    ("exclude", "paths left out"),
    ("source", "code covered and checked for issues"),
    ("include", "paths taken in"),
    ("php", "ini, env, server and const values"),
    ("env", "environment variable for the tests"),
    ("server", "$_SERVER value"),
    ("ini", "php.ini setting"),
    ("const", "PHP constant"),
    ("coverage", "code coverage settings"),
    ("report", "coverage reports"),
    ("logging", "test result logs"),
    ("extensions", "PHPUnit extensions"),
    ("groups", "include or exclude groups"),
    ("group", "a test group"),
];

pub fn phpunit_static(want: &XmlWant, schema: &Schema) -> Option<Vec<Suggestion>> {
    Some(match want {
        XmlWant::Element => {
            let mut out: Vec<Suggestion> = PHPUNIT_ELEMENT_HINTS
                .iter()
                .filter(|(e, _)| {
                    schema.elements.is_empty() || schema.elements.iter().any(|s| s == e)
                })
                .map(|(e, d)| key(e, d))
                .collect();
            for e in &schema.elements {
                if !out.iter().any(|s| &s.label == e) {
                    out.push(key(e, "PHPUnit element"));
                }
            }
            out
        }
        XmlWant::Attribute { tag } if tag == "phpunit" => schema
            .attributes
            .iter()
            .map(|(name, kind)| key(name, kind))
            .collect(),
        XmlWant::Attribute { tag } => {
            let names: &[&str] = match tag.as_str() {
                "env" => &["name", "value", "force"],
                "server" | "ini" | "const" | "var" | "get" | "post" | "cookie" | "request"
                | "files" => &["name", "value"],
                "testsuite" => &["name"],
                "directory" => &["suffix", "prefix", "phpVersion", "phpVersionOperator"],
                "source" => &[
                    "restrictNotices",
                    "restrictWarnings",
                    "ignoreSuppressionOfDeprecations",
                    "baseline",
                ],
                "html" => &["outputDirectory", "lowUpperBound", "highLowerBound"],
                "clover" | "cobertura" | "junit" | "text" | "teamcity" | "testdoxHtml"
                | "testdoxText" => &["outputFile"],
                "bootstrap" => &["class"],
                _ => return None,
            };
            names
                .iter()
                .map(|n| key(n, &format!("<{tag}> attribute")))
                .collect()
        }
        XmlWant::Value { tag, attr, .. } if tag == "phpunit" => {
            let kind = schema
                .attributes
                .iter()
                .find(|(n, _)| n == attr)
                .map(|(_, k)| k.as_str())
                .unwrap_or_default();
            match (attr.as_str(), kind) {
                ("executionOrder", _) => schema
                    .orders
                    .iter()
                    .map(|o| value(o, "execution order"))
                    .collect(),
                ("bootstrap", _) => vec![value("vendor/autoload.php", "Composer autoloader")],
                ("cacheDirectory", _) => vec![value(".phpunit.cache", "default cache folder")],
                ("columns", _) => vec![value("max", "terminal width"), value("80", "")],
                (_, "boolean") => vec![value("true", ""), value("false", "")],
                _ => return None,
            }
        }
        XmlWant::Value { tag, attr, .. } if tag == "testsuite" && attr == "name" => {
            ["Unit", "Feature", "Integration", "Browser", "Arch"]
                .iter()
                .map(|n| value(n, "suite name"))
                .collect()
        }
        XmlWant::Value { tag, attr, .. } if tag == "env" && attr == "force" => {
            vec![
                value("true", "override a value already set"),
                value("false", ""),
            ]
        }
        XmlWant::Value { tag, attr, .. } if tag == "directory" && attr == "suffix" => {
            vec![value("Test.php", ""), value(".php", "")]
        }
        _ => return None,
    })
}

pub fn quoted_at(text: &str, cursor: usize) -> Option<(usize, usize, char)> {
    let cursor = cursor.min(text.len());
    let line_start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..cursor];
    let mut open: Option<(usize, char)> = None;
    let mut escaped = false;
    for (i, c) in before.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (c, open) {
            ('\\', Some(_)) => escaped = true,
            ('\'' | '"' | '`', None) => open = Some((i, c)),
            (q, Some((_, o))) if q == o => open = None,
            _ => {}
        }
    }
    let (at, quote) = open?;
    let start = line_start + at + 1;
    let end = text[cursor..]
        .find([quote, '\n'])
        .map_or(cursor, |i| cursor + i);
    Some((start, end, quote))
}

pub fn js_call(text: &str, cursor: usize) -> Option<(usize, usize, String)> {
    let (start, end, _) = quoted_at(text, cursor)?;
    let before = text[..start - 1].trim_end();
    let before = before.strip_suffix('(')?.trim_end();
    let name_start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$' || c == '.'))
        .map_or(0, |i| i + 1);
    let name = before[name_start..].rsplit('.').next()?.to_owned();
    (!name.is_empty()).then_some((start, end, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> (String, usize) {
        let cursor = text.find('|').unwrap();
        (text.replace('|', ""), cursor)
    }

    fn labels(c: &Completion) -> Vec<String> {
        c.items.iter().map(|s| s.label.clone()).collect()
    }

    const LOCK: &str = r#"{"packages": [{"name": "laravel/framework", "version": "v13.40.0", "description": "The framework", "type": "library"}], "packages-dev": [{"name": "pestphp/pest-plugin", "version": "v4.0.0", "description": "Pest plugin", "type": "composer-plugin"}]}"#;

    #[test]
    fn composer_json_completes_keys_packages_versions_and_scripts() {
        let installed = packages(LOCK);
        assert_eq!(installed.len(), 2);
        let (t, c) = at("{\n  \"|\"\n}");
        assert!(labels(&composer(&t, c, &installed).unwrap()).contains(&"require-dev".to_owned()));
        let (t, c) = at("{\"require\": {\"lar|\"}}");
        let names = labels(&composer(&t, c, &installed).unwrap());
        assert!(
            names.contains(&"laravel/framework".to_owned())
                && names.contains(&"laravel/sanctum".to_owned())
        );
        let (t, c) = at("{\"require\": {\"laravel/framework\": \"|\"}}");
        assert!(labels(&composer(&t, c, &installed).unwrap()).contains(&"^13.40".to_owned()));
        let (t, c) = at("{\"scripts\": {\"post-autoload-dump\": [\"|\"]}}");
        assert!(
            labels(&composer(&t, c, &installed).unwrap())
                .contains(&"@php artisan package:discover --ansi".to_owned())
        );
        let (t, c) = at("{\"config\": {\"allow-plugins\": {\"|\"}}}");
        assert!(
            labels(&composer(&t, c, &installed).unwrap())
                .contains(&"pestphp/pest-plugin".to_owned())
        );
        let (t, c) = at("{\"autoload\": {\"psr-4\": {\"|\"}}}");
        assert!(labels(&composer(&t, c, &installed).unwrap()).contains(&"App\\\\".to_owned()));
    }

    const XSD: &str = r#"<xs:attributeGroup name="configAttributeGroup"><xs:attribute name="colors" type="xs:boolean" default="false"/><xs:attribute name="executionOrder" type="executionOrderType"/><xs:attribute name="bootstrap" type="xs:anyURI"/></xs:attributeGroup><xs:simpleType name="executionOrderType"><xs:restriction base="xs:string"><xs:enumeration value="default"/><xs:enumeration value="random"/></xs:restriction></xs:simpleType><xs:element name="phpunit"/><xs:element name="testsuites"/><xs:element name="env"/>"#;

    #[test]
    fn phpunit_xml_reads_the_installed_schema() {
        let schema = phpunit_schema(XSD);
        assert_eq!(schema.attributes.len(), 3);
        assert_eq!(schema.orders, ["default", "random"]);
        let (t, c) = at("<phpunit col|");
        let (_, _, want) = xml_context(&t, c).unwrap();
        assert_eq!(
            want,
            XmlWant::Attribute {
                tag: "phpunit".into()
            }
        );
        let attrs: Vec<String> = phpunit_static(&want, &schema)
            .unwrap()
            .into_iter()
            .map(|s| s.label)
            .collect();
        assert!(attrs.contains(&"colors".to_owned()));
        let (t, c) = at("<phpunit executionOrder=\"|\">");
        let (s, e, want) = xml_context(&t, c).unwrap();
        assert_eq!((s, e), (c, c));
        let orders: Vec<String> = phpunit_static(&want, &schema)
            .unwrap()
            .into_iter()
            .map(|s| s.label)
            .collect();
        assert_eq!(orders, ["default", "random"]);
        let (t, c) = at("<php>\n  <env name=\"DB_CONNECTION\" value=\"sq|\"/>");
        let (s, _, want) = xml_context(&t, c).unwrap();
        assert_eq!(&t[s..c], "sq");
        assert_eq!(
            want,
            XmlWant::Value {
                tag: "env".into(),
                attr: "value".into(),
                name: Some("DB_CONNECTION".into())
            }
        );
        let (t, c) = at("<testsuite name=\"Unit\">\n  <directory>tests/U|</directory>");
        let (s, e, want) = xml_context(&t, c).unwrap();
        assert_eq!(&t[s..e], "tests/U");
        assert_eq!(
            want,
            XmlWant::Text {
                tag: "directory".into()
            }
        );
        let (t, c) = at("<php>\n  <e|");
        assert_eq!(xml_context(&t, c).unwrap().2, XmlWant::Element);
    }

    #[test]
    fn javascript_calls_and_strings() {
        let (t, c) = at("const url = route('products.|')");
        let (s, e, name) = js_call(&t, c).unwrap();
        assert_eq!((name.as_str(), &t[s..e]), ("route", "products."));
        let (t, c) = at("{{ $t(\"shop.ti|\") }}");
        assert_eq!(js_call(&t, c).unwrap().2, "$t");
        let (t, c) = at("laravel({ input: ['resources/css/|'] })");
        let (s, _, _) = quoted_at(&t, c).unwrap();
        assert_eq!(&t[s..c], "resources/css/");
        let (t, c) = at("const a = 'done'; const b = |");
        assert!(quoted_at(&t, c).is_none());
    }
}
