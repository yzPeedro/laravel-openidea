use crate::Laravel;
use crate::artisan::{self, Command, GLOBAL_OPTIONS};
use crate::devtools;
use crate::index::{Entry, RefKind};
use crate::inspect::{self, Found};
use crate::logs;
use crate::project::{laravel_version, project_of, rel};
use crate::resources;
use crate::runner::{self, Mode};
use crate::sections::{self, Facet, Row, Table, facet, split};
use openidea_plugin::views::{Body, View, ViewEvent, Viewport, Views};
use openidea_plugin::{Error, Invocation, Location, Project, host};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::rc::Rc;

const HUB_PAGE: &str = include_str!("../web/hub.html");
const HISTORY_KEY: &str = "artisan.history";
const HISTORY_SIZE: usize = 20;
const MAX_ROWS: usize = 5000;
const MAX_OUTPUT: usize = 400_000;
const PROJECT_DIRS: [&str; 10] = [
    "app/",
    "routes/",
    "config/",
    "database/",
    "resources/",
    "vendor/",
    "bootstrap/",
    "tests/",
    "lang/",
    "public/",
];

const ICONS: &[(&str, &str)] = &[
    ("laravel", include_str!("../web/icons/laravel.svg")),
    ("console", include_str!("../web/icons/console.svg")),
    ("routing", include_str!("../web/icons/routing.svg")),
    ("database", include_str!("../web/icons/database.svg")),
    (
        "migrations",
        include_str!("../web/icons/folder-migrations.svg"),
    ),
    ("views", include_str!("../web/icons/folder-views.svg")),
    (
        "components",
        include_str!("../web/icons/folder-components.svg"),
    ),
    ("settings", include_str!("../web/icons/settings.svg")),
    ("tune", include_str!("../web/icons/tune.svg")),
    ("i18n", include_str!("../web/icons/i18n.svg")),
    (
        "middleware",
        include_str!("../web/icons/folder-middleware.svg"),
    ),
    ("policy", include_str!("../web/icons/folder-policy.svg")),
    ("queue", include_str!("../web/icons/folder-queue.svg")),
    ("job", include_str!("../web/icons/folder-job.svg")),
    ("tasks", include_str!("../web/icons/folder-tasks.svg")),
    ("event", include_str!("../web/icons/folder-event.svg")),
    ("email", include_str!("../web/icons/email.svg")),
    ("test", include_str!("../web/icons/folder-test.svg")),
    ("log", include_str!("../web/icons/log.svg")),
    ("docker", include_str!("../web/icons/docker.svg")),
    ("controller", include_str!("../web/icons/controller.svg")),
    ("command", include_str!("../web/icons/folder-command.svg")),
    ("http", include_str!("../web/icons/http.svg")),
    ("key", include_str!("../web/icons/key.svg")),
    ("php", include_str!("../web/icons/php.svg")),
    ("api", include_str!("../web/icons/folder-api.svg")),
    ("resource", include_str!("../web/icons/folder-resource.svg")),
    ("class", include_str!("../web/icons/folder-class.svg")),
    ("enum", include_str!("../web/icons/folder-enum.svg")),
    ("rules", include_str!("../web/icons/folder-rules.svg")),
    (
        "container",
        include_str!("../web/icons/folder-container.svg"),
    ),
    ("filter", include_str!("../web/icons/folder-filter.svg")),
    ("seeders", include_str!("../web/icons/folder-seeders.svg")),
    ("hook", include_str!("../web/icons/folder-hook.svg")),
    ("mappings", include_str!("../web/icons/folder-mappings.svg")),
    ("plugin", include_str!("../web/icons/folder-plugin.svg")),
    (
        "functions",
        include_str!("../web/icons/folder-functions.svg"),
    ),
    (
        "connection",
        include_str!("../web/icons/folder-connection.svg"),
    ),
    ("trigger", include_str!("../web/icons/folder-trigger.svg")),
    ("dump", include_str!("../web/icons/folder-dump.svg")),
    ("debug", include_str!("../web/icons/folder-debug.svg")),
    ("packages", include_str!("../web/icons/folder-packages.svg")),
    ("terminal", include_str!("../web/icons/folder-console.svg")),
    ("secure", include_str!("../web/icons/folder-secure.svg")),
    ("robot", include_str!("../web/icons/robot.svg")),
    ("vite", include_str!("../web/icons/vite.svg")),
];

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Request {
    Overview,
    Section { id: String },
    Init,
    Refresh,
    Run { argv: Vec<String> },
    Capture { argv: Vec<String> },
    Files,
    Tail { path: String, size: i64 },
    Open { path: String, line: u32 },
    Tinker { code: String, tab: String },
    Tinkersave { tabs: serde_json::Value },
    Tinkerload,
    Audit { action: String },
    Audittail { size: i64 },
    Agents { action: String },
    Composer { argv: Vec<String> },
}

const TINKER_KEY: &str = "tinker.tabs";

fn quiet(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|l| {
            let t = l.trim();
            !(t.starts_with("Container ")
                || t.starts_with("Network ")
                || t.contains("Pulling")
                || t.is_empty())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Serialize)]
struct Action {
    label: &'static str,
    argv: Vec<String>,
    mode: &'static str,
}

fn action(label: &'static str, argv: &str, mode: &'static str) -> Action {
    Action {
        label,
        argv: argv.split_whitespace().map(str::to_owned).collect(),
        mode,
    }
}

fn project(ctx: &Invocation) -> Result<(Project, String), Error> {
    let project = ctx
        .project()
        .or_else(|| project_of(&ctx.editor()?.path()))
        .ok_or("open a Laravel project first")?;
    let root = project.root()?;
    Ok((project, root))
}

fn history() -> Vec<Vec<String>> {
    host::storage::get(HISTORY_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn remember(argv: &[String]) -> Vec<Vec<String>> {
    let mut recent = history();
    recent.retain(|h| h != argv);
    recent.insert(0, argv.to_vec());
    recent.truncate(HISTORY_SIZE);
    if let Ok(text) = serde_json::to_string(&recent) {
        let _ = host::storage::set(HISTORY_KEY, &text);
    }
    recent
}

fn post(view: &str, value: serde_json::Value) {
    let _ = host::post_view_message(view, &value.to_string());
}

fn denied(e: Error, mode: &Mode) -> String {
    match e {
        Error::Denied(cap) => format!(
            "Allow \"{cap}\" for Laravel OpenIdea in Settings › Plugins to run Artisan with {}",
            mode.label()
        ),
        other => other.to_string(),
    }
}

pub fn project_relative(project: &Project, root: &str, path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut candidates = vec![rel(root, &normalized).to_owned()];
    for dir in PROJECT_DIRS {
        if let Some(at) = normalized.find(&format!("/{dir}")) {
            candidates.push(normalized[at + 1..].to_owned());
        }
    }
    candidates
        .into_iter()
        .find(|c| !c.is_empty() && !c.starts_with('/') && project.read_file(c).is_ok())
}

fn row(root: &str, cells: Vec<String>, location: Option<&Location>) -> Row {
    match location {
        Some(l) if !l.path.is_empty() => Row::at(cells, rel(root, &l.path), l.range.start.line + 1),
        _ => Row::at(cells, "", 0),
    }
}

fn found_rows(found: Vec<Found>) -> Vec<Row> {
    found
        .into_iter()
        .map(|f| Row::at(f.cells, &f.path, f.line))
        .collect()
}

const DEV_SECTIONS: [&str; 4] = ["debug-tools", "debug-calls", "profiles", "ecosystem"];

const CODE_SECTIONS: [&str; 18] = [
    "classes",
    "controllers",
    "requests",
    "api-resources",
    "enums",
    "casts",
    "rules",
    "scopes",
    "observers",
    "listeners",
    "events",
    "providers",
    "bindings",
    "factories",
    "macros",
    "services",
    "frontend",
    "policies",
];

fn table(columns: Vec<&'static str>, rows: Vec<Row>, facets: Vec<Facet>) -> Option<Table> {
    Some(Table {
        columns,
        rows,
        facets,
    })
}

fn head(key: &str, sep: char) -> String {
    key.split_once(sep)
        .map_or_else(|| "—".to_owned(), |(h, _)| h.to_owned())
}

fn section_actions(id: &str, project: &Project) -> Vec<Action> {
    match id {
        "routes" => vec![
            action("route:list", "route:list", "capture"),
            action("Cache routes", "route:cache", "capture"),
            action("Clear route cache", "route:clear", "capture"),
        ],
        "models" => vec![
            action("Prune (dry run)", "model:prune --pretend", "capture"),
            action("New model", "make:model", "terminal"),
        ],
        "database" => vec![
            action("migrate:status", "migrate:status", "capture"),
            action("db:show", "db:show", "capture"),
            action("Migrate", "migrate", "terminal"),
            action("Rollback", "migrate:rollback", "terminal"),
            action("Fresh + seed", "migrate:fresh --seed", "terminal"),
            action("Seed", "db:seed", "terminal"),
        ],
        "views" => vec![
            action("Cache views", "view:cache", "capture"),
            action("Clear compiled views", "view:clear", "capture"),
        ],
        "config" => vec![
            action("Cache config", "config:cache", "capture"),
            action("Clear config cache", "config:clear", "capture"),
            action("config:show app", "config:show app", "capture"),
        ],
        "env" => vec![action("Current environment", "env", "capture")],
        "translations" => vec![action("Publish language files", "lang:publish", "capture")],
        "queues" => {
            let mut actions = vec![
                action("Start worker", "queue:work", "terminal"),
                action("Listen", "queue:listen", "terminal"),
                action("Failed jobs", "queue:failed", "capture"),
                action("Retry failed", "queue:retry all", "terminal"),
                action("Restart workers", "queue:restart", "capture"),
                action("Monitor", "queue:monitor default", "capture"),
            ];
            if project.read_file("config/horizon.php").is_ok() {
                actions.insert(0, action("Horizon", "horizon", "terminal"));
            }
            actions
        }
        "schedule" => vec![
            action("schedule:list", "schedule:list", "capture"),
            action("Run scheduler", "schedule:work", "terminal"),
            action("Run due tasks", "schedule:run", "terminal"),
            action("Test a task", "schedule:test", "terminal"),
        ],
        "events" | "listeners" => vec![
            action("event:list", "event:list", "capture"),
            action("Cache events", "event:cache", "capture"),
            action("Clear event cache", "event:clear", "capture"),
        ],
        "tests" => vec![
            action("Run all tests", "test", "terminal"),
            action("Run in parallel", "test --parallel", "terminal"),
            action("Stop on failure", "test --stop-on-failure", "terminal"),
        ],
        "factories" => vec![
            action("Seed", "db:seed", "terminal"),
            action("New factory", "make:factory", "terminal"),
            action("New seeder", "make:seeder", "terminal"),
        ],
        "providers" => vec![action("about", "about", "capture")],
        "policies" => vec![action("New policy", "make:policy", "terminal")],
        "services" => vec![
            action("Clear cache", "cache:clear", "capture"),
            action("Storage link", "storage:link", "capture"),
            action("db:show", "db:show", "capture"),
        ],
        "observers" => vec![action("New observer", "make:observer", "terminal")],
        "debug-tools" => vec![
            action("Pail (live logs)", "pail", "terminal"),
            action("Dump server", "dump-server", "terminal"),
            action("IDE helper", "ide-helper:generate", "capture"),
        ],
        "profiles" => vec![action("Clockwork clean", "clockwork:clean", "capture")],
        "ecosystem" => {
            let composer = project.read_file("composer.json").unwrap_or_default();
            let has = |p: &str| composer.contains(&format!("\"{p}\""));
            let mut a = vec![action("about", "about", "capture")];
            if has("laravel/horizon") {
                a.extend([
                    action("Horizon", "horizon", "terminal"),
                    action("horizon:status", "horizon:status", "capture"),
                ]);
            }
            if has("laravel/reverb") {
                a.push(action("Reverb", "reverb:start --debug", "terminal"));
            }
            if has("laravel/octane") {
                a.push(action("Octane", "octane:start --watch", "terminal"));
            }
            if has("laravel/wayfinder") {
                a.push(action(
                    "wayfinder:generate",
                    "wayfinder:generate",
                    "capture",
                ));
            }
            if has("laravel/pulse") {
                a.push(action("pulse:check", "pulse:check", "terminal"));
            }
            if has("laravel/sanctum") {
                a.push(action("Prune tokens", "sanctum:prune-expired", "capture"));
            } else {
                a.push(action("install:api", "install:api", "terminal"));
            }
            if has("darkaonline/l5-swagger") {
                a.push(action("Generate OpenAPI", "l5-swagger:generate", "capture"));
            }
            a.push(action(
                "install:broadcasting",
                "install:broadcasting",
                "terminal",
            ));
            a
        }
        "enums" => vec![action("New enum", "make:enum", "terminal")],
        "casts" => vec![action("New cast", "make:cast", "terminal")],
        "rules" => vec![action("New rule", "make:rule", "terminal")],
        "scopes" => vec![action("New scope", "make:scope", "terminal")],
        "requests" => vec![action("New form request", "make:request", "terminal")],
        "api-resources" => vec![action("New resource", "make:resource", "terminal")],
        "controllers" => vec![action("New controller", "make:controller", "terminal")],
        "mail" => vec![
            action("New mailable", "make:mail", "terminal"),
            action("New notification", "make:notification", "terminal"),
        ],
        _ => Vec::new(),
    }
}

fn page() -> String {
    let icons: serde_json::Map<String, serde_json::Value> = ICONS
        .iter()
        .map(|(k, v)| ((*k).to_owned(), json!(v.trim())))
        .collect();
    HUB_PAGE.replace(
        "\"__ICONS__\"",
        &serde_json::Value::Object(icons).to_string(),
    )
}

impl Laravel {
    fn artisan_commands(&mut self, project: &Project, root: &str, fresh: bool) -> Rc<Vec<Command>> {
        if fresh {
            self.commands.remove(root);
        }
        self.commands
            .entry(root.to_owned())
            .or_insert_with(|| Rc::new(artisan::discover(project, root)))
            .clone()
    }

    fn send_commands(&mut self, view: &str, ctx: &Invocation, fresh: bool) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        let commands = self.artisan_commands(&project, &root, fresh);
        let globals: Vec<_> = GLOBAL_OPTIONS
            .iter()
            .map(|(name, description)| json!({ "name": name, "value": *name == "env", "array": false, "description": description }))
            .collect();
        let mode = runner::detect(&project);
        post(
            view,
            json!({ "type": "commands", "commands": *commands, "globals": globals, "runner": mode.label(), "history": history() }),
        );
        Ok(())
    }

    fn run_terminal(
        &mut self,
        view: &str,
        ctx: &Invocation,
        argv: Vec<String>,
    ) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let title = format!(
            "artisan {}",
            argv.first().map(String::as_str).unwrap_or_default()
        );
        let error = host::open_terminal(&title, mode.program(), &mode.args(&argv, true))
            .err()
            .map(|e| denied(e, &mode));
        let recent = if error.is_none() {
            remember(&argv)
        } else {
            history()
        };
        post(
            view,
            json!({ "type": "ran", "argv": argv, "error": error, "history": recent }),
        );
        Ok(())
    }

    fn capture(&mut self, view: &str, ctx: &Invocation, argv: Vec<String>) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let title = format!("php artisan {}", argv.join(" "));
        let task = host::begin_task(&title, true);
        let mut colored = argv.clone();
        if !colored.iter().any(|a| a == "--ansi" || a == "--no-ansi") {
            colored.push("--ansi".into());
        }
        let result = project.run_process(mode.program(), &mode.args(&colored, false), None);
        drop(task);
        let message = match result {
            Ok(out) => {
                let mut text = out.stdout;
                if !out.stderr.trim().is_empty() {
                    text.push_str(&out.stderr);
                }
                text.truncate(MAX_OUTPUT);
                remember(&argv);
                json!({ "type": "output", "argv": argv, "ok": out.status == Some(0), "text": text, "runner": mode.label() })
            }
            Err(e) => {
                json!({ "type": "output", "argv": argv, "ok": false, "text": denied(e, &mode), "runner": mode.label() })
            }
        };
        post(view, message);
        Ok(())
    }

    fn log_files(&mut self, view: &str, ctx: &Invocation) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        let channels: Vec<String> = self
            .cache
            .get(RefKind::LogChannel, &project, &root)
            .iter()
            .map(|e| e.key.clone())
            .collect();
        post(
            view,
            json!({ "type": "files", "files": logs::files(&project, &channels) }),
        );
        Ok(())
    }

    fn tail(&mut self, view: &str, ctx: &Invocation, path: &str, size: i64) -> Result<(), Error> {
        if !path.starts_with("storage/logs/") || path.contains("..") {
            return Err(Error::Invalid(format!("{path} is not a log file")));
        }
        let (project, _) = project(ctx)?;
        match project.read_file(path) {
            Ok(text) if text.len() as i64 == size => {}
            Ok(text) => post(
                view,
                json!({ "type": "entries", "path": path, "size": text.len(), "entries": logs::parse(&text) }),
            ),
            Err(e) => post(
                view,
                json!({ "type": "error", "message": format!("{path}: {e}") }),
            ),
        }
        Ok(())
    }

    fn open_file(
        &mut self,
        view: &str,
        ctx: &Invocation,
        path: &str,
        line: u32,
    ) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        match project_relative(&project, &root, path) {
            Some(file) => {
                if let Err(e) = host::open_location(&file, line.saturating_sub(1), 0) {
                    post(view, json!({ "type": "error", "message": e.to_string() }));
                }
            }
            None => post(
                view,
                json!({ "type": "error", "message": format!("{path} is not a file of this project") }),
            ),
        }
        Ok(())
    }

    fn overview(&mut self, view: &str, ctx: &Invocation) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        let env_value = |key: &str| {
            [".env", ".env.example"].iter().find_map(|f| {
                let text = project.read_file(f).ok()?;
                text.lines().find_map(|l| {
                    l.strip_prefix(&format!("{key}="))
                        .map(|v| v.trim().trim_matches('"').to_owned())
                })
            })
        };
        let composer = project
            .read_file("composer.json")
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok());
        let php = composer
            .as_ref()
            .and_then(|c| c["require"]["php"].as_str().map(str::to_owned));
        let packages: Vec<String> = composer
            .as_ref()
            .map(|c| {
                ["require", "require-dev"]
                    .iter()
                    .filter_map(|k| c[*k].as_object())
                    .flat_map(|o| o.keys().cloned())
                    .filter(|k| k.contains('/'))
                    .collect()
            })
            .unwrap_or_default();
        let mut count = |kind: RefKind| self.cache.get(kind, &project, &root).len();
        let counts = json!({
            "routes": count(RefKind::RouteAction).max(count(RefKind::Route)),
            "models": count(RefKind::Model),
            "tables": count(RefKind::Table),
            "views": count(RefKind::View),
            "components": count(RefKind::Component) + count(RefKind::Livewire),
            "config": count(RefKind::Config),
            "translations": count(RefKind::Translation),
            "middleware": count(RefKind::Middleware),
            "abilities": count(RefKind::Ability),
            "openapi": count(RefKind::OpenApi),
        });
        let mut counts = counts;
        let scan = resources::scan(&project, &root);
        if let Some(map) = counts.as_object_mut() {
            for (k, v) in sections::counts(&scan, &project, &root) {
                map.insert(k.to_owned(), json!(v));
            }
            map.insert(
                "scopes".into(),
                json!(self.cache.get(RefKind::Scope, &project, &root).len()),
            );
        }
        let commands = self.artisan_commands(&project, &root, false).len();
        let mode = runner::detect(&project);
        post(
            view,
            json!({
                "type": "overview",
                "name": env_value("APP_NAME").unwrap_or_else(|| root.rsplit(['/', '\\']).next().unwrap_or_default().to_owned()),
                "env": env_value("APP_ENV"),
                "url": env_value("APP_URL"),
                "laravel": laravel_version(&project),
                "php": php,
                "runner": mode.label(),
                "docker": !matches!(mode, Mode::Php),
                "packages": packages,
                "counts": counts,
                "commands": commands,
            }),
        );
        Ok(())
    }

    fn rows_of(
        &mut self,
        project: &Project,
        root: &str,
        kind: RefKind,
        cells: impl Fn(&Entry) -> Vec<String>,
    ) -> Vec<Row> {
        self.cache
            .get(kind, project, root)
            .iter()
            .take(MAX_ROWS)
            .map(|e| row(root, cells(e), Some(&e.location)))
            .collect()
    }

    fn index_section(&mut self, project: &Project, root: &str, id: &str) -> Option<Table> {
        let text = |s: &Option<String>| s.clone().unwrap_or_default();
        match id {
            "routes" => {
                let controllers = self.cache.get(RefKind::Controller, project, root);
                let mut seen = BTreeSet::new();
                let mut rows = Vec::new();
                for e in self.cache.get(RefKind::RouteAction, project, root).iter() {
                    let detail = text(&e.detail);
                    let (head_part, name) =
                        detail.split_once(" · ").unwrap_or((detail.as_str(), ""));
                    let (method, uri) = head_part.split_once(' ').unwrap_or((head_part, ""));
                    seen.insert((e.location.path.clone(), e.location.range.start.line));
                    let method_location = controllers
                        .iter()
                        .find(|c| c.key == e.key.replace('@', "."))
                        .map(|c| c.location.clone());
                    let controller = e
                        .key
                        .split('@')
                        .next()
                        .unwrap_or_default()
                        .rsplit('\\')
                        .next()
                        .unwrap_or_default()
                        .to_owned();
                    rows.push(row(
                        root,
                        vec![
                            method.into(),
                            uri.into(),
                            name.into(),
                            e.key.replace('@', "::"),
                            prefix(uri),
                            controller,
                        ],
                        method_location.as_ref().or(Some(&e.location)),
                    ));
                }
                for e in self.cache.get(RefKind::Route, project, root).iter() {
                    if seen.insert((e.location.path.clone(), e.location.range.start.line)) {
                        let detail = text(&e.detail);
                        let (method, uri) = detail.split_once(' ').unwrap_or((detail.as_str(), ""));
                        rows.push(row(
                            root,
                            vec![
                                method.into(),
                                uri.into(),
                                e.key.clone(),
                                "closure".into(),
                                prefix(uri),
                                "closure".into(),
                            ],
                            Some(&e.location),
                        ));
                    }
                }
                rows.sort_by(|a, b| {
                    a.cells[1]
                        .cmp(&b.cells[1])
                        .then_with(|| a.cells[0].cmp(&b.cells[0]))
                });
                let tests: Vec<(String, String)> =
                    crate::project::find(project, root, "tests/**/*.php", 20_000)
                        .into_iter()
                        .filter_map(|p| {
                            let text = project.read_file(&p).ok()?;
                            Some((
                                rel(root, &p)
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or_default()
                                    .to_owned(),
                                text,
                            ))
                        })
                        .collect();
                for r in &mut rows {
                    let name = r.cells[2].clone();
                    let uri = r.cells[1].clone();
                    let hits: Vec<&str> = tests
                        .iter()
                        .filter(|(_, t)| {
                            (!name.is_empty()
                                && (t.contains(&format!("'{name}'"))
                                    || t.contains(&format!("\"{name}\""))))
                                || (uri.len() > 1 && literals(t).any(|l| uri_matches(&uri, l)))
                        })
                        .map(|(f, _)| f.as_str())
                        .collect();
                    r.cells.push(match hits.as_slice() {
                        [] => "—".into(),
                        [one] => (*one).to_owned(),
                        [first, rest @ ..] => format!("{first} +{}", rest.len()),
                    });
                    r.cells
                        .push(if hits.is_empty() { "no" } else { "yes" }.into());
                }
                table(
                    vec![
                        "Method",
                        "URI",
                        "Name",
                        "Action",
                        "Prefix",
                        "Controller",
                        "Tests",
                        "Tested",
                    ],
                    rows,
                    vec![
                        split("Method"),
                        facet("Prefix"),
                        facet("Controller"),
                        facet("Tested"),
                    ],
                )
            }
            "models" => {
                let relations = self.cache.get(RefKind::Relation, project, root);
                let scopes = self.cache.get(RefKind::Scope, project, root);
                let rows = self.rows_of(project, root, RefKind::Model, |e| {
                    let prefix = format!("{}.", e.key);
                    let rels: Vec<&str> = relations
                        .iter()
                        .filter_map(|r| r.key.strip_prefix(&prefix))
                        .collect();
                    let kinds: BTreeSet<String> = relations
                        .iter()
                        .filter(|r| r.key.starts_with(&prefix))
                        .filter_map(|r| r.detail.as_deref())
                        .map(|d| d.split('(').next().unwrap_or(d).to_owned())
                        .collect();
                    let sc: Vec<&str> = scopes
                        .iter()
                        .filter_map(|r| r.key.strip_prefix(&prefix))
                        .collect();
                    vec![
                        e.key.clone(),
                        text(&e.detail).trim_start_matches("table ").to_owned(),
                        rels.join(", "),
                        kinds.into_iter().collect::<Vec<_>>().join(", "),
                        sc.join(", "),
                    ]
                });
                table(
                    vec!["Model", "Table", "Relations", "Relation types", "Scopes"],
                    rows,
                    vec![split("Relation types")],
                )
            }
            "database" => {
                let columns = self.cache.get(RefKind::Column, project, root);
                let rows = self.rows_of(project, root, RefKind::Table, |e| {
                    let prefix = format!("{}.", e.key);
                    let cols: Vec<&str> = columns
                        .iter()
                        .filter_map(|c| c.key.strip_prefix(&prefix))
                        .collect();
                    let detail = text(&e.detail);
                    let year = detail
                        .rsplit('/')
                        .next()
                        .and_then(|f| f.get(..4))
                        .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or("—")
                        .to_owned();
                    vec![
                        e.key.clone(),
                        cols.len().to_string(),
                        cols.join(", "),
                        detail,
                        year,
                    ]
                });
                table(
                    vec!["Table", "Columns", "Fields", "Migration", "Year"],
                    rows,
                    vec![facet("Year")],
                )
            }
            "views" => {
                let rows = self.rows_of(project, root, RefKind::View, |e| {
                    let folder = e
                        .key
                        .rsplit_once('.')
                        .map_or_else(|| "(root)".to_owned(), |(f, _)| f.to_owned());
                    let namespace = e
                        .key
                        .split_once("::")
                        .map_or_else(|| "app".to_owned(), |(n, _)| n.to_owned());
                    vec![e.key.clone(), text(&e.detail), folder, namespace]
                });
                table(
                    vec!["View", "File", "Folder", "Namespace"],
                    rows,
                    vec![facet("Folder"), facet("Namespace")],
                )
            }
            "components" => {
                let mut rows = self.rows_of(project, root, RefKind::Component, |e| {
                    vec![format!("<x-{}>", e.key), "Blade".into(), text(&e.detail)]
                });
                rows.extend(self.rows_of(project, root, RefKind::Livewire, |e| {
                    vec![
                        format!("<livewire:{}>", e.key),
                        "Livewire".into(),
                        text(&e.detail),
                    ]
                }));
                table(vec!["Tag", "Kind", "File"], rows, vec![facet("Kind")])
            }
            "config" => {
                let rows = self.rows_of(project, root, RefKind::Config, |e| {
                    vec![e.key.clone(), text(&e.detail), head(&e.key, '.')]
                });
                table(vec!["Key", "Value", "File"], rows, vec![facet("File")])
            }
            "env" => {
                let rows = self.rows_of(project, root, RefKind::Env, |e| {
                    vec![e.key.clone(), text(&e.detail), head(&e.key, '_')]
                });
                table(
                    vec!["Variable", "Example value", "Group"],
                    rows,
                    vec![facet("Group")],
                )
            }
            "translations" => {
                let rows = self.rows_of(project, root, RefKind::Translation, |e| {
                    let detail = text(&e.detail);
                    let (locale, value) = detail.split_once(": ").unwrap_or((detail.as_str(), ""));
                    let group = if e.key.contains(' ') || !e.key.contains('.') {
                        "json".to_owned()
                    } else {
                        head(&e.key, '.')
                    };
                    vec![
                        e.key.clone(),
                        locale.into(),
                        value.trim_matches('\'').into(),
                        group,
                    ]
                });
                table(
                    vec!["Key", "Locale", "Text", "Group"],
                    rows,
                    vec![facet("Locale"), facet("Group")],
                )
            }
            "middleware" => {
                let rows = self.rows_of(project, root, RefKind::Middleware, |e| {
                    let detail = text(&e.detail);
                    let kind = if detail.starts_with('[') || detail.contains(',') {
                        "group"
                    } else {
                        "alias"
                    };
                    vec![e.key.clone(), detail, kind.into()]
                });
                table(vec!["Alias", "Source", "Kind"], rows, vec![facet("Kind")])
            }
            "queues" => {
                let mut rows: Vec<Row> = self
                    .cache
                    .get(RefKind::Queue, project, root)
                    .iter()
                    .map(|e| {
                        row(
                            root,
                            vec![
                                e.key.clone(),
                                "connection".into(),
                                "—".into(),
                                "—".into(),
                                "—".into(),
                                text(&e.detail),
                            ],
                            Some(&e.location),
                        )
                    })
                    .collect();
                rows.extend(found_rows(inspect::jobs(project, root)));
                table(
                    vec!["Name", "Kind", "Queue", "Tries", "Timeout", "Source"],
                    rows,
                    vec![facet("Kind"), facet("Queue")],
                )
            }
            "schedule" => table(
                vec!["Task", "Frequency", "Defined in"],
                found_rows(inspect::scheduled_tasks(project)),
                vec![facet("Defined in")],
            ),
            "mail" => table(
                vec!["Class", "Kind", "Channels", "File"],
                found_rows(inspect::messages(project, root)),
                vec![facet("Kind"), split("Channels")],
            ),
            "tests" => table(
                vec!["File", "Suite", "Tests"],
                found_rows(inspect::tests(project, root)),
                vec![facet("Suite")],
            ),
            _ => None,
        }
    }

    fn section(&mut self, view: &str, ctx: &Invocation, id: &str) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        let built = if DEV_SECTIONS.contains(&id) {
            Some(match id {
                "debug-tools" => devtools::tools(&project),
                "debug-calls" => devtools::debug_calls(&project, &root),
                "ecosystem" => {
                    let config: Vec<(String, String)> = self
                        .cache
                        .get(RefKind::Config, &project, &root)
                        .iter()
                        .map(|e| (e.key.clone(), e.detail.clone().unwrap_or_default()))
                        .collect();
                    let mut count = |k: RefKind| self.cache.get(k, &project, &root).len();
                    let counts = [
                        ("token abilities", count(RefKind::TokenAbility)),
                        ("socialite", count(RefKind::SocialiteDriver)),
                        ("roles", count(RefKind::Role)),
                        ("permissions", count(RefKind::Permission)),
                        ("livewire", count(RefKind::Livewire)),
                        ("inertia", count(RefKind::InertiaPage)),
                    ];
                    devtools::ecosystem(&project, &root, &config, &counts)
                }
                _ => devtools::profiles(&project, &root),
            })
        } else if CODE_SECTIONS.contains(&id) {
            let scan = resources::scan(&project, &root);
            sections::build(id, &scan, &project, &root, &mut self.cache)
        } else {
            self.index_section(&project, &root, id)
        };
        let Some(mut t) = built else {
            return Err(Error::NotFound(format!("section {id}")));
        };
        t.rows.truncate(MAX_ROWS * 4);
        post(
            view,
            json!({ "type": "section", "id": id, "columns": t.columns, "rows": t.rows, "facets": t.facets, "actions": section_actions(id, &project) }),
        );
        Ok(())
    }
}

fn literals(text: &str) -> impl Iterator<Item = &str> {
    text.split(['\'', '"'])
        .skip(1)
        .step_by(2)
        .filter(|l| l.contains('/') && !l.contains(' ') && l.len() < 200)
}

fn uri_matches(pattern: &str, literal: &str) -> bool {
    let literal = literal.split('?').next().unwrap_or_default();
    let p: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let l: Vec<&str> = literal.trim_matches('/').split('/').collect();
    p.len() == l.len()
        && p.iter()
            .zip(&l)
            .all(|(a, b)| (a.starts_with('{') && !b.is_empty()) || a == b)
}

fn prefix(uri: &str) -> String {
    uri.trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty() && !s.starts_with('{'))
        .map_or_else(|| "/".to_owned(), |s| format!("/{s}"))
}

impl Laravel {
    fn tinker(&mut self, view: &str, ctx: &Invocation, code: &str, tab: &str) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let task = host::begin_task("Tinker", true);
        let args = mode.php_args(&["-r".into(), devtools::script(devtools::TINKER_PHP)]);
        let result = project.run_process(mode.program(), &args, Some(code));
        drop(task);
        let message = match result {
            Ok(out) => {
                let (records, noise) = devtools::records(&out.stdout);
                let mut noise = noise;
                let err = quiet(&out.stderr);
                if !err.is_empty() {
                    noise.push_str(&err);
                }
                json!({ "type": "tinker", "tab": tab, "records": records, "noise": noise, "ok": out.status == Some(0), "runner": mode.label() })
            }
            Err(e) => {
                json!({ "type": "tinker", "tab": tab, "records": [], "noise": denied(e, &mode), "ok": false, "runner": mode.label() })
            }
        };
        post(view, message);
        Ok(())
    }

    fn audit(&mut self, view: &str, ctx: &Invocation, action: &str) -> Result<(), Error> {
        if !["install", "uninstall", "clear", "status"].contains(&action) {
            return Err(Error::Invalid(format!("unknown audit action {action}")));
        }
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let task = host::begin_task(&format!("Live audit: {action}"), true);
        let args = mode.php_args(&[
            "-r".into(),
            devtools::script(devtools::INSTALL_PHP),
            action.into(),
        ]);
        let stdin = (action == "install").then_some(devtools::RECORDER_PHP);
        let result = project.run_process(mode.program(), &args, stdin);
        drop(task);
        let message = match result {
            Ok(out) => {
                let (records, noise) = devtools::records(&out.stdout);
                let last = records.last().cloned().unwrap_or_default();
                let error = last["error"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| (last.is_null()).then(|| format!("{noise}{}", quiet(&out.stderr))));
                json!({ "type": "auditstatus", "action": action, "active": last["active"].as_bool(), "error": error, "runner": mode.label(), "files": devtools::status_from_files(&project) })
            }
            Err(e) => {
                json!({ "type": "auditstatus", "action": action, "active": null, "error": denied(e, &mode), "runner": mode.label() })
            }
        };
        post(view, message);
        Ok(())
    }

    fn audit_tail(&mut self, view: &str, ctx: &Invocation, size: i64) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        match project.read_file(devtools::AUDIT_LOG) {
            Ok(text) if text.len() as i64 == size => {}
            Ok(text) => {
                let (records, reset) = devtools::tail(&text, size.max(0) as usize);
                post(
                    view,
                    json!({ "type": "audit", "size": text.len(), "reset": reset, "records": records }),
                );
            }
            Err(_) => post(
                view,
                json!({ "type": "audit", "size": 0, "reset": true, "records": [], "missing": true }),
            ),
        }
        Ok(())
    }
}

impl Laravel {
    fn agents(&mut self, view: &str, ctx: &Invocation, action: &str) -> Result<(), Error> {
        if !["agents", "agents-off", "agents-status"].contains(&action) {
            return Err(Error::Invalid(format!("unknown agents action {action}")));
        }
        let (project, root) = project(ctx)?;
        let mode = runner::detect(&project);
        let (command, args) = mode.mcp_server(&root, runner::compose_file(&project));
        let payload = json!({
            "mcp": devtools::MCP_PHP,
            "command": command,
            "args": args,
            "guidelines": devtools::GUIDELINES,
        })
        .to_string();
        let task = host::begin_task("AI agents", true);
        let run = mode.php_args(&[
            "-r".into(),
            devtools::script(devtools::INSTALL_PHP),
            action.into(),
        ]);
        let result = project.run_process(
            mode.program(),
            &run,
            (action == "agents").then_some(payload.as_str()),
        );
        drop(task);
        let lock = project.read_file("composer.lock").unwrap_or_default();
        let message = match result {
            Ok(out) => {
                let (records, noise) = devtools::records(&out.stdout);
                let mut last = records.last().cloned().unwrap_or_default();
                if last.is_null() {
                    last =
                        json!({ "ok": false, "error": format!("{noise}{}", quiet(&out.stderr)) });
                }
                last["type"] = json!("agents");
                last["action"] = json!(action);
                last["command"] = json!(format!("{command} {}", args.join(" ")));
                last["boostVersion"] = json!(devtools::lock_version(&lock, "laravel/boost"));
                last["runner"] = json!(mode.label());
                last
            }
            Err(e) => {
                json!({ "type": "agents", "action": action, "ok": false, "error": denied(e, &mode) })
            }
        };
        post(view, message);
        Ok(())
    }

    fn composer(&mut self, view: &str, ctx: &Invocation, argv: Vec<String>) -> Result<(), Error> {
        let allowed = [
            "require",
            "update",
            "install",
            "remove",
            "dump-autoload",
            "show",
            "outdated",
        ];
        if !argv.first().is_some_and(|a| allowed.contains(&a.as_str())) {
            return Err(Error::Invalid(
                "composer command not allowed from the hub".into(),
            ));
        }
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let (program, args) = mode.composer(&argv);
        let error = host::open_terminal(&format!("composer {}", argv.join(" ")), program, &args)
            .err()
            .map(|e| denied(e, &mode));
        post(
            view,
            json!({ "type": "ran", "argv": argv, "error": error, "history": history() }),
        );
        Ok(())
    }
}

impl Views for Laravel {
    fn render(
        &mut self,
        view: &str,
        _viewport: Option<Viewport>,
        _ctx: &Invocation,
    ) -> Result<View, Error> {
        match view {
            "laravel" => Ok(View::new(Body::Html(page()))),
            other => Err(Error::NotFound(format!("view {other}"))),
        }
    }

    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        let ViewEvent::Message(text) = event else {
            return Ok(());
        };
        let request: Request =
            serde_json::from_str(&text).map_err(|e| Error::Invalid(e.to_string()))?;
        match request {
            Request::Overview => self.overview(view, ctx),
            Request::Section { id } => self.section(view, ctx, &id),
            Request::Init => self.send_commands(view, ctx, false),
            Request::Refresh => self.send_commands(view, ctx, true),
            Request::Run { argv } => self.run_terminal(view, ctx, argv),
            Request::Capture { argv } => self.capture(view, ctx, argv),
            Request::Files => self.log_files(view, ctx),
            Request::Tail { path, size } => self.tail(view, ctx, &path, size),
            Request::Open { path, line } => self.open_file(view, ctx, &path, line),
            Request::Tinker { code, tab } => self.tinker(view, ctx, &code, &tab),
            Request::Tinkersave { tabs } => {
                let _ = host::storage::set(TINKER_KEY, &tabs.to_string());
                Ok(())
            }
            Request::Tinkerload => {
                let tabs: serde_json::Value = host::storage::get(TINKER_KEY)
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or(serde_json::Value::Null);
                post(view, json!({ "type": "tinkertabs", "tabs": tabs }));
                Ok(())
            }
            Request::Audit { action } => self.audit(view, ctx, &action),
            Request::Audittail { size } => self.audit_tail(view, ctx, size),
            Request::Agents { action } => self.agents(view, ctx, &action),
            Request::Composer { argv } => self.composer(view, ctx, argv),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_route_uris_in_tests() {
        assert!(uri_matches("/orders/{order}", "/orders/7"));
        assert!(uri_matches("/api/products", "api/products?page=2"));
        assert!(!uri_matches("/orders/{order}/checkout", "/orders/7"));
        assert!(!uri_matches("/orders", "/orders/7"));
        let text = "$this->get('/orders/1')->assertOk(); $this->post(\"/cart\");";
        assert_eq!(literals(text).collect::<Vec<_>>(), ["/orders/1", "/cart"]);
    }

    #[test]
    fn maps_container_paths_to_project_files() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file("app/Http/Controllers/UserController.php", "<?php");
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let r = |p: &str| project_relative(&project, "/project", p);
        assert_eq!(
            r("/var/www/html/app/Http/Controllers/UserController.php").as_deref(),
            Some("app/Http/Controllers/UserController.php")
        );
        assert_eq!(
            r("/project/app/Http/Controllers/UserController.php").as_deref(),
            Some("app/Http/Controllers/UserController.php")
        );
        assert_eq!(r("/var/www/html/app/Missing.php"), None);
        assert!(page().contains("\"laravel\":\"<svg"));
    }
}
