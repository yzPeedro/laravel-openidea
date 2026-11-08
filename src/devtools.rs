use crate::php::{Kind, tokenize};
use crate::project::{Lines, find, rel};
use crate::sections::{Row, Table, facet};
use openidea_plugin::Project;
use serde_json::{Value, json};

pub const TINKER_PHP: &str = include_str!("../php/tinker.php");
pub const INSTALL_PHP: &str = include_str!("../php/install.php");
pub const RECORDER_PHP: &str = include_str!("../php/Recorder.php");
pub const MCP_PHP: &str = include_str!("../php/mcp.php");
pub const SELECTION_PHP: &str = include_str!("../php/selection.php");
pub const CREATE_FILE_PHP: &str = include_str!("../php/create_file.php");
pub const TOOLS_PHP: &str = include_str!("../php/tools.php");
pub const CACHE_PHP: &str = include_str!("../php/cache.php");
const CACHE_MARKER: &str = "@@openidea-cache@@";

pub fn cache_reply(stdout: &str) -> Option<Value> {
    let at = stdout.rfind(CACHE_MARKER)?;
    let line = stdout[at + CACHE_MARKER.len()..].lines().next()?;
    serde_json::from_str(line).ok()
}
pub const GUIDELINES: &str = include_str!("../php/guidelines.md");
pub const AUDIT_LOG: &str = "storage/logs/openidea-audit.jsonl";
const MARKER: &str = "@@openidea@@";
const MAX_RECORDS: usize = 4000;

pub fn script(source: &str) -> String {
    source
        .trim_start()
        .strip_prefix("<?php")
        .unwrap_or(source)
        .to_owned()
}

pub fn records(stdout: &str) -> (Vec<Value>, String) {
    let mut out = Vec::new();
    let mut noise = String::new();
    for line in stdout.lines() {
        match line.find(MARKER) {
            Some(at) => {
                match serde_json::from_str::<Value>(&line[at + MARKER.len()..]) {
                    Ok(v) => out.push(v),
                    Err(e) => crate::audit::warn("reading a helper script's output", e),
                }
                noise.push_str(&line[..at]);
            }
            None if !line.trim().is_empty() => {
                noise.push_str(line);
                noise.push('\n');
            }
            None => {}
        }
    }
    (out, noise)
}

pub fn fit_records(mut records: Vec<Value>, budget: usize) -> (Vec<Value>, usize) {
    let sizes: Vec<usize> = records.iter().map(|r| r.to_string().len() + 1).collect();
    let mut total: usize = sizes.iter().sum();
    let mut drop = 0;
    while total > budget && drop < sizes.len() {
        total -= sizes[drop];
        drop += 1;
    }
    records.drain(..drop);
    (records, drop)
}

pub fn tail(text: &str, from: usize) -> (Vec<Value>, bool) {
    let reset = from > text.len() || !text.is_char_boundary(from);
    let start = if reset { 0 } else { from };
    let mut lines: Vec<&str> = text[start..].lines().collect();
    let reset = reset || from == 0;
    if lines.len() > MAX_RECORDS {
        lines.drain(..lines.len() - MAX_RECORDS);
    }
    let parsed = lines
        .into_iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    (parsed, reset)
}

struct Tool {
    package: &'static str,
    name: &'static str,
    kind: &'static str,
    config: &'static str,
    hint: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool {
        package: "barryvdh/laravel-debugbar",
        name: "Debugbar",
        kind: "profiler",
        config: "config/debugbar.php",
        hint: "DEBUGBAR_ENABLED; stored requests in storage/debugbar are listed in Profiles",
    },
    Tool {
        package: "fruitcake/laravel-debugbar",
        name: "Debugbar",
        kind: "profiler",
        config: "config/debugbar.php",
        hint: "DEBUGBAR_ENABLED; stored requests in storage/debugbar are listed in Profiles",
    },
    Tool {
        package: "itsgoingd/clockwork",
        name: "Clockwork",
        kind: "profiler",
        config: "config/clockwork.php",
        hint: "CLOCKWORK_ENABLE; requests in storage/clockwork are listed in Profiles; UI at /clockwork",
    },
    Tool {
        package: "laravel/telescope",
        name: "Telescope",
        kind: "profiler",
        config: "config/telescope.php",
        hint: "TELESCOPE_ENABLED; UI at /telescope",
    },
    Tool {
        package: "laravel/pulse",
        name: "Pulse",
        kind: "monitoring",
        config: "config/pulse.php",
        hint: "Dashboard at /pulse",
    },
    Tool {
        package: "laravel/horizon",
        name: "Horizon",
        kind: "queues",
        config: "config/horizon.php",
        hint: "Dashboard at /horizon; run with php artisan horizon",
    },
    Tool {
        package: "laravel/pail",
        name: "Pail",
        kind: "logs",
        config: "",
        hint: "php artisan pail tails every log channel",
    },
    Tool {
        package: "spatie/laravel-ray",
        name: "Ray",
        kind: "dumps",
        config: "ray.php",
        hint: "ray() sends to the Ray app; RAY_ENABLED, RAY_HOST for Docker",
    },
    Tool {
        package: "laradumps/laradumps",
        name: "LaraDumps",
        kind: "dumps",
        config: "laradumps.yaml",
        hint: "ds() sends to the LaraDumps app; ds:init configures it",
    },
    Tool {
        package: "beyondcode/laravel-dump-server",
        name: "Dump Server",
        kind: "dumps",
        config: "config/debug-server.php",
        hint: "php artisan dump-server collects dump() output",
    },
    Tool {
        package: "symfony/var-dumper",
        name: "VarDumper",
        kind: "dumps",
        config: "",
        hint: "dump() and dd(); captured in Live audit when the recorder is on",
    },
    Tool {
        package: "spatie/laravel-ignition",
        name: "Ignition",
        kind: "errors",
        config: "config/ignition.php",
        hint: "Error pages with solutions",
    },
    Tool {
        package: "nunomaduro/collision",
        name: "Collision",
        kind: "errors",
        config: "",
        hint: "CLI error reporting",
    },
    Tool {
        package: "barryvdh/laravel-ide-helper",
        name: "IDE Helper",
        kind: "tooling",
        config: "config/ide-helper.php",
        hint: "ide-helper:generate, ide-helper:models, ide-helper:meta",
    },
    Tool {
        package: "larastan/larastan",
        name: "Larastan",
        kind: "static analysis",
        config: "phpstan.neon",
        hint: "vendor/bin/phpstan analyse",
    },
    Tool {
        package: "nunomaduro/larastan",
        name: "Larastan",
        kind: "static analysis",
        config: "phpstan.neon",
        hint: "vendor/bin/phpstan analyse",
    },
    Tool {
        package: "laravel/pint",
        name: "Pint",
        kind: "code style",
        config: "pint.json",
        hint: "vendor/bin/pint",
    },
    Tool {
        package: "rector/rector",
        name: "Rector",
        kind: "refactoring",
        config: "rector.php",
        hint: "vendor/bin/rector",
    },
    Tool {
        package: "laravel/boost",
        name: "Laravel Boost",
        kind: "AI",
        config: "boost.json",
        hint: "MCP server and AI guidelines: php artisan boost:install",
    },
    Tool {
        package: "pestphp/pest",
        name: "Pest",
        kind: "testing",
        config: "tests/Pest.php",
        hint: "php artisan test",
    },
    Tool {
        package: "laravel/dusk",
        name: "Dusk",
        kind: "testing",
        config: "",
        hint: "php artisan dusk",
    },
];

pub fn lock_version(lock: &str, package: &str) -> Option<String> {
    let at = lock.find(&format!("\"name\": \"{package}\""))?;
    let rest = &lock[at..];
    let v = rest.find("\"version\": \"")? + 12;
    let end = rest[v..].find('"')? + v;
    Some(rest[v..end].trim_start_matches('v').to_owned())
}

pub fn tools(project: &Project) -> Table {
    let lock = project.read_file("composer.lock").unwrap_or_default();
    let composer = project.read_file("composer.json").unwrap_or_default();
    let env = project.read_file(".env.example").unwrap_or_default();
    let mut rows = Vec::new();
    for t in TOOLS {
        let version = lock_version(&lock, t.package);
        let required = composer.contains(&format!("\"{}\"", t.package));
        if version.is_none() && !required {
            continue;
        }
        let config =
            (!t.config.is_empty() && project.read_file(t.config).is_ok()).then_some(t.config);
        let flag = t
            .hint
            .split(|c: char| !(c.is_ascii_uppercase() || c == '_'))
            .find(|w| w.len() > 4 && w.contains('_'))
            .and_then(|key| {
                env.lines()
                    .find_map(|l| l.strip_prefix(&format!("{key}=")))
                    .map(|v| format!("{key}={v}"))
            });
        rows.push(Row::at(
            vec![
                t.name.into(),
                t.kind.into(),
                version.unwrap_or_else(|| "required".into()),
                if composer
                    .find("\"require-dev\"")
                    .is_some_and(|at| composer[at..].contains(&format!("\"{}\"", t.package)))
                {
                    "dev".into()
                } else {
                    "prod".into()
                },
                config.unwrap_or("—").into(),
                flag.unwrap_or_else(|| "—".into()),
                t.hint.into(),
            ],
            config.unwrap_or(""),
            1,
        ));
    }
    Table {
        columns: vec![
            "Tool",
            "Kind",
            "Version",
            "Scope",
            "Config",
            "Example env",
            "How to use",
        ],
        rows,
        facets: vec![facet("Kind"), facet("Scope")],
    }
}

const DEBUG_FUNCTIONS: &[&str] = &[
    "dd",
    "dump",
    "ddd",
    "ray",
    "rd",
    "ds",
    "dsd",
    "dsq",
    "dsr",
    "var_dump",
    "print_r",
    "var_export",
    "clock",
    "debug",
    "error_log",
    "dump_server",
];

pub fn debug_calls(project: &Project, root: &str) -> Table {
    let mut paths = Vec::new();
    for glob in [
        "app/**/*.php",
        "routes/**/*.php",
        "resources/views/**/*.php",
        "database/**/*.php",
        "config/**/*.php",
    ] {
        paths.extend(find(project, root, glob, 20_000));
    }
    paths.sort();
    paths.dedup();
    let mut rows = Vec::new();
    for p in paths {
        let Ok(text) = crate::project::read_listed(project, &p) else {
            continue;
        };
        if !DEBUG_FUNCTIONS
            .iter()
            .any(|f| text.contains(&format!("{f}(")))
            && !text.contains("@dd")
            && !text.contains("@dump")
        {
            continue;
        }
        let path = rel(root, &p).to_owned();
        let lines = Lines::new(&text);
        let tokens = tokenize(&text, 0, text.len());
        for (i, t) in tokens.iter().enumerate() {
            let Kind::Ident(name) = &t.kind else {
                continue;
            };
            let lower = name.trim_start_matches('\\').to_ascii_lowercase();
            if !DEBUG_FUNCTIONS.contains(&lower.as_str())
                || !tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Open('('))
            {
                continue;
            }
            let prev = i.checked_sub(1).map(|k| &tokens[k].kind);
            let method = matches!(prev, Some(Kind::ObjectOp));
            if matches!(prev, Some(Kind::DoubleColon | Kind::At))
                || matches!(prev, Some(Kind::Ident(f)) if f.eq_ignore_ascii_case("function"))
                || (method && !matches!(lower.as_str(), "dd" | "dump" | "ddd" | "ray"))
                || (lower == "debug" && !method && !text.contains("Debugbar"))
            {
                continue;
            }
            let line = lines.position(&text, t.start).line + 1;
            let snippet = text
                .lines()
                .nth(line as usize - 1)
                .unwrap_or_default()
                .trim()
                .chars()
                .take(140)
                .collect::<String>();
            rows.push(Row::at(
                vec![
                    if method {
                        format!("->{lower}()")
                    } else {
                        format!("{lower}()")
                    },
                    snippet,
                    format!("{path}:{line}"),
                    folder_kind(&path).into(),
                ],
                &path,
                line,
            ));
        }
        if path.ends_with(".blade.php") {
            for (n, l) in text.lines().enumerate() {
                for d in ["@dd(", "@dump(", "@ray(", "@ds("] {
                    if l.contains(d) {
                        rows.push(Row::at(
                            vec![
                                d.trim_end_matches('(').into(),
                                l.trim().chars().take(140).collect(),
                                format!("{path}:{}", n + 1),
                                "view".into(),
                            ],
                            &path,
                            n as u32 + 1,
                        ));
                    }
                }
            }
        }
    }
    Table {
        columns: vec!["Call", "Code", "Where", "Area"],
        rows,
        facets: vec![facet("Call"), facet("Area")],
    }
}

fn folder_kind(path: &str) -> &'static str {
    if path.ends_with(".blade.php") {
        "view"
    } else if path.starts_with("routes/") {
        "routes"
    } else if path.starts_with("database/") {
        "database"
    } else if path.starts_with("config/") {
        "config"
    } else {
        "app"
    }
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| {
        v.as_str()
            .and_then(|s| s.trim_end_matches("ms").trim().parse().ok())
    })
}

pub fn profiles(project: &Project, root: &str) -> Table {
    let mut rows = Vec::new();
    let mut files = find(project, root, "storage/clockwork/*.json", 5000);
    files.sort();
    files.reverse();
    for f in files.into_iter().take(300) {
        let Ok(text) = crate::project::read_listed(project, &f) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let queries = v["databaseQueries"].as_array().map_or(0, Vec::len);
        let slow: f64 = v["databaseQueries"]
            .as_array()
            .map(|q| q.iter().filter_map(|x| num(&x["duration"])).sum())
            .unwrap_or_default();
        let kind = if v["type"].as_str() == Some("command") {
            "command"
        } else if v["type"].as_str() == Some("queue-job") {
            "job"
        } else {
            "request"
        };
        let target = match kind {
            "command" => v["commandName"].as_str().unwrap_or_default().to_owned(),
            "job" => v["jobName"].as_str().unwrap_or_default().to_owned(),
            _ => format!(
                "{} {}",
                v["method"].as_str().unwrap_or_default(),
                v["uri"].as_str().unwrap_or_default()
            ),
        };
        rows.push(Row::at(
            vec![
                time_label(num(&v["time"]).unwrap_or_default()),
                "Clockwork".into(),
                kind.into(),
                target,
                v["responseStatus"]
                    .as_i64()
                    .map_or_else(|| "—".into(), |s| s.to_string()),
                num(&v["responseDuration"]).map_or_else(|| "—".into(), |d| format!("{d:.1} ms")),
                format!("{queries} ({slow:.1} ms)"),
                v["controller"].as_str().unwrap_or_default().to_owned(),
            ],
            "",
            0,
        ));
    }
    let mut files = find(project, root, "storage/debugbar/*.json", 5000);
    files.sort();
    files.reverse();
    for f in files.into_iter().take(300) {
        let Ok(text) = crate::project::read_listed(project, &f) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let meta = &v["__meta"];
        let queries = v["queries"]["nb_statements"]
            .as_i64()
            .or_else(|| v["queries"]["count"].as_i64())
            .unwrap_or_default();
        let qms = num(&v["queries"]["accumulated_duration"])
            .map(|s| s * 1000.0)
            .unwrap_or_default();
        rows.push(Row::at(
            vec![
                meta["datetime"].as_str().unwrap_or_default().to_owned(),
                "Debugbar".into(),
                "request".into(),
                format!(
                    "{} {}",
                    meta["method"].as_str().unwrap_or_default(),
                    meta["uri"].as_str().unwrap_or_default()
                ),
                v["request"]["status_code"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| v["request"]["status_code"].as_i64().map(|s| s.to_string()))
                    .unwrap_or_else(|| "—".into()),
                v["time"]["duration_str"].as_str().unwrap_or("—").to_owned(),
                format!("{queries} ({qms:.1} ms)"),
                v["route"]["uses"].as_str().unwrap_or_default().to_owned(),
            ],
            "",
            0,
        ));
    }
    Table {
        columns: vec![
            "When", "Source", "Kind", "Target", "Status", "Duration", "Queries", "Action",
        ],
        rows,
        facets: vec![facet("Source"), facet("Kind"), facet("Status")],
    }
}

fn time_label(t: f64) -> String {
    if t <= 0.0 {
        return "—".into();
    }
    let secs = t as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02} UTC")
}

fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

struct Eco {
    package: &'static str,
    name: &'static str,
    area: &'static str,
    hint: &'static str,
}

const ECOSYSTEM: &[Eco] = &[
    Eco {
        package: "laravel/sanctum",
        name: "Sanctum",
        area: "auth",
        hint: "API tokens and SPA auth · php artisan install:api",
    },
    Eco {
        package: "laravel/passport",
        name: "Passport",
        area: "auth",
        hint: "OAuth2 server",
    },
    Eco {
        package: "laravel/fortify",
        name: "Fortify",
        area: "auth",
        hint: "Headless auth backend",
    },
    Eco {
        package: "laravel/socialite",
        name: "Socialite",
        area: "auth",
        hint: "OAuth logins",
    },
    Eco {
        package: "laravel/workos",
        name: "WorkOS AuthKit",
        area: "auth",
        hint: "WorkOS sign-in for the starter kits",
    },
    Eco {
        package: "laravel/wayfinder",
        name: "Wayfinder",
        area: "frontend",
        hint: "Typed TS functions for routes and actions · php artisan wayfinder:generate",
    },
    Eco {
        package: "inertiajs/inertia-laravel",
        name: "Inertia",
        area: "frontend",
        hint: "Server-driven SPA",
    },
    Eco {
        package: "livewire/livewire",
        name: "Livewire",
        area: "frontend",
        hint: "Reactive Blade components",
    },
    Eco {
        package: "livewire/volt",
        name: "Volt",
        area: "frontend",
        hint: "Single-file Livewire",
    },
    Eco {
        package: "livewire/flux",
        name: "Flux",
        area: "frontend",
        hint: "Livewire UI kit",
    },
    Eco {
        package: "laravel/folio",
        name: "Folio",
        area: "frontend",
        hint: "Page-based routing",
    },
    Eco {
        package: "laravel/precognition",
        name: "Precognition",
        area: "frontend",
        hint: "Live validation",
    },
    Eco {
        package: "laravel/horizon",
        name: "Horizon",
        area: "queues",
        hint: "Redis queue dashboard · php artisan horizon",
    },
    Eco {
        package: "laravel/reverb",
        name: "Reverb",
        area: "realtime",
        hint: "WebSocket server · php artisan reverb:start",
    },
    Eco {
        package: "laravel/octane",
        name: "Octane",
        area: "server",
        hint: "FrankenPHP, Swoole or RoadRunner · php artisan octane:start",
    },
    Eco {
        package: "laravel/sail",
        name: "Sail",
        area: "server",
        hint: "Docker environment · vendor/bin/sail up",
    },
    Eco {
        package: "laravel/pulse",
        name: "Pulse",
        area: "monitoring",
        hint: "Performance dashboard at /pulse",
    },
    Eco {
        package: "laravel/telescope",
        name: "Telescope",
        area: "monitoring",
        hint: "Debug assistant at /telescope",
    },
    Eco {
        package: "laravel/nightwatch",
        name: "Nightwatch",
        area: "monitoring",
        hint: "Hosted monitoring · php artisan nightwatch:agent",
    },
    Eco {
        package: "laravel/scout",
        name: "Scout",
        area: "search",
        hint: "Full-text search",
    },
    Eco {
        package: "laravel/cashier",
        name: "Cashier (Stripe)",
        area: "billing",
        hint: "Subscriptions",
    },
    Eco {
        package: "laravel/cashier-paddle",
        name: "Cashier (Paddle)",
        area: "billing",
        hint: "Subscriptions",
    },
    Eco {
        package: "laravel/pennant",
        name: "Pennant",
        area: "features",
        hint: "Feature flags",
    },
    Eco {
        package: "laravel/prompts",
        name: "Prompts",
        area: "cli",
        hint: "Beautiful CLI forms",
    },
    Eco {
        package: "laravel/boost",
        name: "Boost",
        area: "AI",
        hint: "MCP server and AI guidelines · php artisan boost:install",
    },
    Eco {
        package: "laravel/mcp",
        name: "Laravel MCP",
        area: "AI",
        hint: "Build MCP servers in the app",
    },
    Eco {
        package: "laravel/breeze",
        name: "Breeze",
        area: "starter kit",
        hint: "Legacy starter kit",
    },
    Eco {
        package: "laravel/jetstream",
        name: "Jetstream",
        area: "starter kit",
        hint: "Legacy starter kit",
    },
    Eco {
        package: "filament/filament",
        name: "Filament",
        area: "admin",
        hint: "Admin panels",
    },
    Eco {
        package: "laravel/nova",
        name: "Nova",
        area: "admin",
        hint: "Admin panels",
    },
    Eco {
        package: "spatie/laravel-permission",
        name: "Permission",
        area: "auth",
        hint: "Roles and permissions",
    },
    Eco {
        package: "spatie/laravel-data",
        name: "Laravel Data",
        area: "data",
        hint: "Data objects",
    },
    Eco {
        package: "spatie/laravel-medialibrary",
        name: "Media Library",
        area: "data",
        hint: "Files on models",
    },
    Eco {
        package: "darkaonline/l5-swagger",
        name: "L5 Swagger",
        area: "API docs",
        hint: "OpenAPI from attributes · php artisan l5-swagger:generate",
    },
    Eco {
        package: "dedoc/scramble",
        name: "Scramble",
        area: "API docs",
        hint: "OpenAPI from code, at /docs/api",
    },
];

fn env_value(env: &str, key: &str) -> Option<String> {
    env.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .map(|v| v.trim().trim_matches('"').to_owned())
}

pub fn ecosystem(
    project: &Project,
    root: &str,
    config: &[(String, String)],
    counts: &[(&str, usize)],
) -> Table {
    let lock = project.read_file("composer.lock").unwrap_or_default();
    let composer = project.read_file("composer.json").unwrap_or_default();
    let npm = project.read_file("package.json").unwrap_or_default();
    let env = project.read_file(".env.example").unwrap_or_default();
    let conf_prefix = |p: &str| -> Vec<String> {
        let mut out: Vec<String> = config
            .iter()
            .filter_map(|(k, _)| k.strip_prefix(p))
            .filter_map(|rest| rest.split('.').next())
            .map(str::to_owned)
            .collect();
        out.dedup();
        out
    };
    let count = |name: &str| {
        counts
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(0, |(_, c)| *c)
    };
    let files = |glob: &str| find(project, root, glob, 5000).len();
    let mut rows = Vec::new();
    for e in ECOSYSTEM {
        let version = lock_version(&lock, e.package);
        if version.is_none() && !composer.contains(&format!("\"{}\"", e.package)) {
            continue;
        }
        let details = match e.package {
            "laravel/sanctum" => format!(
                "{} token abilities · stateful {} · {}",
                count("token abilities"),
                env_value(&env, "SANCTUM_STATEFUL_DOMAINS").unwrap_or_else(|| "default".into()),
                if project.read_file("config/sanctum.php").is_ok() {
                    "published config"
                } else {
                    "package config"
                }
            ),
            "laravel/wayfinder" => format!(
                "{} action files · {} route files generated",
                files("resources/js/actions/**/*.ts"),
                files("resources/js/routes/**/*.ts")
            ),
            "laravel/horizon" => format!(
                "environments {} · path /{}",
                conf_prefix("horizon.environments.").join(", "),
                env_value(&env, "HORIZON_PATH").unwrap_or_else(|| "horizon".into())
            ),
            "laravel/reverb" => format!(
                "host {} · port {}",
                env_value(&env, "REVERB_HOST").unwrap_or_else(|| "—".into()),
                env_value(&env, "REVERB_PORT").unwrap_or_else(|| "—".into())
            ),
            "laravel/octane" => format!(
                "server {}",
                env_value(&env, "OCTANE_SERVER").unwrap_or_else(|| "—".into())
            ),
            "laravel/socialite" => format!("{} drivers", count("socialite")),
            "laravel/scout" => format!(
                "driver {}",
                env_value(&env, "SCOUT_DRIVER").unwrap_or_else(|| "—".into())
            ),
            "laravel/fortify" => format!("features {}", conf_prefix("fortify.features.").len()),
            "spatie/laravel-permission" => format!(
                "{} roles · {} permissions",
                count("roles"),
                count("permissions")
            ),
            "livewire/livewire" => format!("{} components", count("livewire")),
            "inertiajs/inertia-laravel" => format!("{} pages", count("inertia")),
            "laravel/telescope" => format!(
                "TELESCOPE_ENABLED={}",
                env_value(&env, "TELESCOPE_ENABLED").unwrap_or_else(|| "—".into())
            ),
            _ => String::new(),
        };
        rows.push(Row::at(
            vec![
                e.name.into(),
                e.area.into(),
                version.unwrap_or_else(|| "required".into()),
                details,
                e.hint.into(),
            ],
            "",
            0,
        ));
    }
    let kit = if npm.contains("\"@inertiajs/react\"") {
        Some("React starter kit (Inertia, TypeScript)")
    } else if npm.contains("\"@inertiajs/vue3\"") {
        Some("Vue starter kit (Inertia, TypeScript)")
    } else if npm.contains("\"@inertiajs/svelte\"") {
        Some("Svelte starter kit (Inertia)")
    } else if composer.contains("\"livewire/flux\"") {
        Some("Livewire starter kit (Flux, Volt)")
    } else {
        None
    };
    if let Some(kit) = kit {
        let auth = if composer.contains("\"laravel/workos\"") {
            "WorkOS AuthKit"
        } else if composer.contains("\"laravel/fortify\"") {
            "Fortify"
        } else {
            "built-in auth"
        };
        let shadcn = project.read_file("components.json").is_ok();
        rows.insert(
            0,
            Row::at(
                vec![
                    kit.into(),
                    "starter kit".into(),
                    "—".into(),
                    format!(
                        "auth via {auth}{}",
                        if shadcn { " · shadcn/ui" } else { "" }
                    ),
                    "laravel new --react / --vue / --livewire".into(),
                ],
                "",
                0,
            ),
        );
    }
    for (name, pkg) in [
        ("Vite", "\"vite\""),
        ("Tailwind CSS", "\"tailwindcss\""),
        ("TypeScript", "\"typescript\""),
        ("Laravel Echo", "\"laravel-echo\""),
        ("Ziggy", "\"ziggy-js\""),
    ] {
        if npm.contains(pkg) {
            rows.push(Row::at(
                vec![
                    name.into(),
                    "frontend (npm)".into(),
                    npm_version(&npm, pkg).unwrap_or_default(),
                    String::new(),
                    String::new(),
                ],
                "package.json",
                1,
            ));
        }
    }
    Table {
        columns: vec!["Package", "Area", "Version", "Details", "How to use"],
        rows,
        facets: vec![facet("Area")],
    }
}

fn npm_version(npm: &str, quoted: &str) -> Option<String> {
    let at = npm.find(&format!("{quoted}:"))?;
    let rest = &npm[at + quoted.len() + 1..];
    let start = rest.find('"')? + 1;
    let end = rest[start..].find('"')? + start;
    Some(rest[start..end].to_owned())
}

pub fn status_from_files(project: &Project) -> Value {
    let recorder = project.read_file("storage/openidea/Recorder.php").is_ok();
    let hooked = project
        .read_file("vendor/autoload.php")
        .is_ok_and(|t| t.contains("openidea-recorder"));
    json!({ "recorder": recorder, "hooked": hooked })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_records_are_trimmed_to_fit_a_message() {
        let records: Vec<Value> = (0..100)
            .map(|i| json!({ "seq": i, "pad": "x".repeat(100) }))
            .collect();
        let (kept, dropped) = fit_records(records, 2_000);
        assert!(dropped > 0 && kept.len() + dropped == 100);
        assert_eq!(kept.last().unwrap()["seq"], 99);
        assert!(kept.iter().map(|r| r.to_string().len() + 1).sum::<usize>() <= 2_000);
    }

    #[test]
    fn parses_marked_records_and_tails() {
        let (r, noise) = records(
            "Notice x\n@@openidea@@{\"kind\":\"done\"}\nwarn @@openidea@@{\"kind\":\"dump\"}\n",
        );
        assert_eq!(r.len(), 2);
        assert_eq!(noise, "Notice x\nwarn ");
        let text = "{\"a\":1}\n{\"a\":2}\nnot json\n";
        let (all, reset) = tail(text, 0);
        assert_eq!((all.len(), reset), (2, true));
        let (after, reset) = tail(text, 8);
        assert_eq!((after.len(), reset), (1, false));
        let (rotated, reset) = tail(text, 999);
        assert_eq!((rotated.len(), reset), (2, true));
        assert_eq!(time_label(0.0), "—");
        assert_eq!(time_label(1_700_000_000.0), "2023-11-14 22:13:20 UTC");
        assert!(script(TINKER_PHP).contains("__openideaEmit"));
        assert!(!script(INSTALL_PHP).starts_with("<?php"));
    }

    #[test]
    fn finds_leftover_debug_calls() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file("app/Http/Controllers/A.php", "<?php\nclass A {\n public function x() {\n  dd($x);\n  $q->dump();\n  Log::debug('ok');\n  ray('hi');\n }\n public function dump() {}\n}\n");
        host.add_file("resources/views/a.blade.php", "<div>@dump($user)</div>\n");
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let t = debug_calls(&project, "/project");
        let calls: Vec<(String, String)> = t
            .rows
            .iter()
            .map(|r| (r.cells[0].clone(), r.cells[2].clone()))
            .collect();
        assert_eq!(
            calls,
            [
                ("dd()".to_owned(), "app/Http/Controllers/A.php:4".to_owned()),
                (
                    "->dump()".to_owned(),
                    "app/Http/Controllers/A.php:5".to_owned()
                ),
                (
                    "ray()".to_owned(),
                    "app/Http/Controllers/A.php:7".to_owned()
                ),
                (
                    "@dump".to_owned(),
                    "resources/views/a.blade.php:1".to_owned()
                ),
            ]
        );
    }
}
