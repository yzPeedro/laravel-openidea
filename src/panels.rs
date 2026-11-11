use crate::Laravel;
use crate::artisan::{self, Command, GLOBAL_OPTIONS};
use crate::audit::{self, Logged};
use crate::devtools;
use crate::index::{Entry, RefKind};
use crate::inspect::{self, Found};
use crate::logs;
use crate::project::{laravel_version, project_of, rel};
use crate::resources;
use crate::runner::{self, Mode};
use crate::sections::{self, Facet, Row, Table, facet, split};
use openidea_plugin::views::{View, ViewEvent, Viewport, Views};
use openidea_plugin::{Error, Invocation, Location, Project, host};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::rc::Rc;

pub struct RouteTest {
    pub file: String,
    pub content: String,
    pub pest: bool,
    pub auth: bool,
}

const HISTORY_KEY: &str = "artisan.history";
const HISTORY_SIZE: usize = 20;
const MAX_ROWS: usize = 5000;
const MAX_OUTPUT: usize = 400_000;
const MAX_MESSAGE: usize = 900_000;
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

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Request {
    Overview,
    Runtime,
    Routeinfo {
        method: String,
        uri: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        action: String,
    },
    Sendrequest {
        method: String,
        uri: String,
        #[serde(default)]
        query: String,
        #[serde(default)]
        body: serde_json::Value,
        #[serde(default)]
        user: String,
        #[serde(default)]
        json: bool,
        #[serde(default)]
        rollback: bool,
    },
    Gentest {
        method: String,
        uri: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        action: String,
    },
    Copy {
        text: String,
    },
    Diagram,
    Securityroutes,
    Indexcheck {
        sqls: Vec<String>,
    },
    Addindex {
        table: String,
        column: String,
    },
    Hello,
    Openeditor {
        id: String,
    },
    Search {
        query: String,
    },
    Model {
        name: String,
    },
    Section {
        id: String,
    },
    Init,
    Refresh,
    Run {
        argv: Vec<String>,
    },
    Capture {
        argv: Vec<String>,
    },
    Files,
    Tail {
        path: String,
        size: i64,
    },
    Open {
        path: String,
        line: u32,
    },
    Tinker {
        code: String,
        tab: String,
    },
    Tinkersave {
        tabs: serde_json::Value,
    },
    Tinkerload,
    Audit {
        action: String,
    },
    Audittail {
        size: i64,
    },
    Agents {
        action: String,
    },
    Composer {
        argv: Vec<String>,
    },
    Sqlload,
    Sqlsave {
        state: serde_json::Value,
    },
    Sqlcopy {
        code: String,
    },
    Sqlinsert {
        code: String,
    },
    Php2sql {
        code: String,
        seq: u64,
    },
    Cache {
        action: String,
        #[serde(default)]
        store: String,
        #[serde(default)]
        page: u32,
        #[serde(default)]
        size: u32,
        #[serde(default)]
        search: String,
        #[serde(default)]
        key: String,
    },
    Schema {
        action: String,
        #[serde(default)]
        table: String,
        #[serde(default)]
        page: u32,
        #[serde(default)]
        size: u32,
        #[serde(default)]
        sql: String,
        #[serde(default)]
        bindings: Vec<serde_json::Value>,
        #[serde(default)]
        connection: String,
    },
}

#[derive(Serialize, Clone, Copy)]
struct SchemaRequest<'a> {
    action: &'a str,
    table: &'a str,
    page: u32,
    size: u32,
    sql: &'a str,
    bindings: &'a [serde_json::Value],
    connection: &'a str,
}

#[derive(Serialize)]
struct CacheRequest<'a> {
    action: &'a str,
    store: &'a str,
    page: u32,
    size: u32,
    search: &'a str,
    key: &'a str,
}

const TINKER_KEY: &str = "tinker.tabs";
const SQL_KEY: &str = "sql.state";

pub fn indent_snippet(code: &str, indent: &str) -> String {
    let mut out = String::new();
    for (i, line) in code.trim_end().lines().enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(indent);
            }
        }
        out.push_str(line);
    }
    out
}

fn line_indent(text: &str, line: u32) -> String {
    text.lines()
        .nth(line as usize)
        .unwrap_or_default()
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

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
        .or_warn("reading the Artisan history")
        .flatten()
        .and_then(|s| serde_json::from_str(&s).or_warn("parsing the Artisan history"))
        .unwrap_or_default()
}

fn remember(argv: &[String]) -> Vec<Vec<String>> {
    let mut recent = history();
    recent.retain(|h| h != argv);
    recent.insert(0, argv.to_vec());
    recent.truncate(HISTORY_SIZE);
    if let Ok(text) = serde_json::to_string(&recent) {
        host::storage::set(HISTORY_KEY, &text).or_warn("saving the Artisan history");
    }
    recent
}

fn post(view: &str, mut value: serde_json::Value) {
    crate::hubtabs::tag(&mut value);
    let kind = value["type"].as_str().unwrap_or_default();
    let problem = match kind {
        "error" => value["message"].as_str(),
        "sqlresult" if value["ok"] == false => value["message"].as_str(),
        "php2sql" => None,
        _ => value["error"].as_str(),
    };
    if let Some(problem) = problem.filter(|p| !p.trim().is_empty()) {
        audit::warn(&format!("hub {kind}"), problem);
    }
    host::post_view_message(view, &value.to_string())
        .or_warn(&format!("posting {kind} to the {view} view"));
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

const SEARCHABLE: [&str; 32] = [
    "routes",
    "controllers",
    "requests",
    "api-resources",
    "middleware",
    "models",
    "scopes",
    "observers",
    "casts",
    "enums",
    "database",
    "factories",
    "views",
    "components",
    "frontend",
    "translations",
    "classes",
    "providers",
    "bindings",
    "rules",
    "macros",
    "config",
    "env",
    "services",
    "policies",
    "queues",
    "schedule",
    "events",
    "listeners",
    "mail",
    "tests",
    "debug-calls",
];
const SEARCH_ROWS: usize = 6;

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

fn table_keys(t: &crate::index::database::SchemaTable) -> (String, String) {
    let join = |v: Vec<String>| {
        if v.is_empty() {
            "—".to_owned()
        } else {
            v.join(", ")
        }
    };
    let indexes = t
        .indexes
        .iter()
        .filter(|i| i.kind != "primary")
        .map(|i| format!("{} ({})", i.columns.join("+"), i.kind))
        .collect();
    let foreigns = t
        .foreigns
        .iter()
        .map(|f| format!("{} → {}.{}", f.column, f.on, f.references))
        .collect();
    (join(indexes), join(foreigns))
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
        let title = format!("artisan {}", argv.join(" "));
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
        let result = crate::runner::run(&project, &mode, &mode.args(&colored, false), None);
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
                audit::failed(&format!("hub artisan {}", argv.join(" ")), &e);
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
        let scan = self.cached_scan(&project, &root);
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
                let cached_actions: std::collections::BTreeMap<String, String> = project
                    .read_file(crate::index::routes::CACHE)
                    .map(|t| {
                        crate::index::routes::cached(&t)
                            .into_iter()
                            .filter_map(|r| Some((r.name, r.action?)))
                            .collect()
                    })
                    .unwrap_or_default();
                let pages = crate::index::folio::pages(project, root);
                let page_paths: BTreeSet<&str> =
                    pages.iter().map(|p| p.location.path.as_str()).collect();
                for e in self.cache.get(RefKind::Route, project, root).iter() {
                    if page_paths.contains(e.location.path.as_str()) {
                        continue;
                    }
                    if seen.insert((e.location.path.clone(), e.location.range.start.line)) {
                        let detail = text(&e.detail);
                        let (method, uri) = detail.split_once(' ').unwrap_or((detail.as_str(), ""));
                        if e.location.path.ends_with(crate::index::routes::CACHE) {
                            let action = cached_actions
                                .get(&e.key)
                                .cloned()
                                .unwrap_or_else(|| "closure".into());
                            let controller = action
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
                                    e.key.clone(),
                                    action.replace('@', "::"),
                                    prefix(uri),
                                    if controller.is_empty() {
                                        "cached".into()
                                    } else {
                                        controller
                                    },
                                ],
                                Some(&e.location),
                            ));
                            continue;
                        }
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
                for page in pages {
                    if seen.insert((page.location.path.clone(), page.location.range.start.line)) {
                        rows.push(row(
                            root,
                            vec![
                                "GET".into(),
                                page.uri.clone(),
                                page.name.clone().unwrap_or_default(),
                                "Folio page".into(),
                                prefix(&page.uri),
                                "folio".into(),
                            ],
                            Some(&page.location),
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
                            let text = crate::project::read_listed(project, &p).ok()?;
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
                let schema = crate::index::database::schema(project, root);
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
                    let (indexes, foreigns) = schema
                        .table(&e.key)
                        .map_or_else(|| ("—".to_owned(), "—".to_owned()), table_keys);
                    vec![
                        e.key.clone(),
                        cols.len().to_string(),
                        cols.join(", "),
                        indexes,
                        foreigns,
                        detail,
                        year,
                    ]
                });
                table(
                    vec![
                        "Table",
                        "Columns",
                        "Fields",
                        "Indexes",
                        "Foreign keys",
                        "Migration",
                        "Year",
                    ],
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
                let locale_of = |e: &Entry| {
                    let d = e.detail.as_deref().unwrap_or_default();
                    d.split_once(": ").map_or(d, |(l, _)| l).to_owned()
                };
                let entries = self.cache.get(RefKind::Translation, project, root);
                let all: BTreeSet<String> = entries.iter().map(locale_of).collect();
                let mut have: std::collections::BTreeMap<String, BTreeSet<String>> =
                    std::collections::BTreeMap::new();
                for e in entries.iter() {
                    have.entry(e.key.clone()).or_default().insert(locale_of(e));
                }
                let missing: std::collections::BTreeMap<String, String> = have
                    .into_iter()
                    .map(|(k, locales)| {
                        let gone: Vec<&str> =
                            all.difference(&locales).map(String::as_str).collect();
                        let shown = if gone.is_empty() {
                            "—".to_owned()
                        } else {
                            gone.join(", ")
                        };
                        (k, shown)
                    })
                    .collect();
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
                        missing.get(&e.key).cloned().unwrap_or_else(|| "—".into()),
                    ]
                });
                table(
                    vec!["Key", "Locale", "Text", "Group", "Missing in"],
                    rows,
                    vec![facet("Locale"), facet("Group"), split("Missing in")],
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
        let built = self.build_section(&project, &root, id, &mut None);
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

    fn hub_search(&mut self, view: &str, ctx: &Invocation, query: &str) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        let needle = query.trim().to_lowercase();
        if needle.chars().count() < 2 {
            post(
                view,
                json!({ "type": "searchresults", "query": query, "groups": [] }),
            );
            return Ok(());
        }
        let fresh = self.hub_tables.as_ref().is_some_and(|(r, _)| *r == root);
        if !fresh {
            let _task = host::begin_task("Laravel: indexing the hub for search", false);
            let mut scan = None;
            let mut tables = Vec::new();
            for id in SEARCHABLE {
                if let Some(t) = self.build_section(&project, &root, id, &mut scan) {
                    tables.push((id, t));
                }
            }
            self.hub_tables = Some((root.clone(), Rc::new(tables)));
        }
        let Some((_, tables)) = self.hub_tables.clone() else {
            return Ok(());
        };
        let groups: Vec<serde_json::Value> = tables
            .iter()
            .filter_map(|(id, t)| {
                let hits: Vec<&sections::Row> = t
                    .rows
                    .iter()
                    .filter(|r| r.cells.iter().any(|c| c.to_lowercase().contains(&needle)))
                    .collect();
                (!hits.is_empty()).then(|| {
                    json!({
                        "id": id,
                        "total": hits.len(),
                        "columns": t.columns,
                        "rows": hits.into_iter().take(SEARCH_ROWS).collect::<Vec<_>>(),
                    })
                })
            })
            .collect();
        post(
            view,
            json!({ "type": "searchresults", "query": query, "groups": groups }),
        );
        Ok(())
    }

    fn build_section(
        &mut self,
        project: &Project,
        root: &str,
        id: &str,
        scan: &mut Option<Rc<resources::Scan>>,
    ) -> Option<Table> {
        let (project, root) = (project, root.to_owned());
        if id == "security" {
            return Some(crate::securityscan::section(project, &root));
        }
        if id == "unused" {
            return Some(self.unused_section(project, &root));
        }
        if id == "upgrade" {
            let installed = self.installed_major(project, &root);
            return Some(crate::upgrade::section(project, &root, installed));
        }
        if DEV_SECTIONS.contains(&id) {
            Some(match id {
                "debug-tools" => devtools::tools(project),
                "debug-calls" => devtools::debug_calls(project, &root),
                "ecosystem" => {
                    let config: Vec<(String, String)> = self
                        .cache
                        .get(RefKind::Config, project, &root)
                        .iter()
                        .map(|e| (e.key.clone(), e.detail.clone().unwrap_or_default()))
                        .collect();
                    let mut count = |k: RefKind| self.cache.get(k, project, &root).len();
                    let counts = [
                        ("token abilities", count(RefKind::TokenAbility)),
                        ("socialite", count(RefKind::SocialiteDriver)),
                        ("roles", count(RefKind::Role)),
                        ("permissions", count(RefKind::Permission)),
                        ("livewire", count(RefKind::Livewire)),
                        ("inertia", count(RefKind::InertiaPage)),
                    ];
                    devtools::ecosystem(project, &root, &config, &counts)
                }
                _ => devtools::profiles(project, &root),
            })
        } else if CODE_SECTIONS.contains(&id) {
            if scan.is_none() {
                *scan = Some(self.cached_scan(project, &root));
            }
            let scan = scan.clone().unwrap_or_default();
            sections::build(id, &scan, project, &root, &mut self.cache)
        } else {
            self.index_section(project, &root, id)
        }
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
        let result = crate::runner::run(&project, &mode, &args, Some(code));
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
                audit::failed("hub tinker", &e);
                json!({ "type": "tinker", "tab": tab, "records": [], "noise": denied(e, &mode), "ok": false, "runner": mode.label() })
            }
        };
        post(view, message);
        Ok(())
    }

    fn cache_store(
        &mut self,
        view: &str,
        ctx: &Invocation,
        request: &CacheRequest,
    ) -> Result<(), Error> {
        if !["list", "value", "forget", "flush"].contains(&request.action) {
            return Err(Error::Invalid(format!(
                "unknown cache action {}",
                request.action
            )));
        }
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let task = host::begin_task("Reading the cache", true);
        let args = mode.php_args(&["-r".into(), devtools::script(devtools::CACHE_PHP)]);
        let input = serde_json::to_string(request).map_err(|e| Error::Failed(e.to_string()))?;
        let result = crate::runner::run(&project, &mode, &args, Some(&input));
        drop(task);
        let mut message = match result {
            Ok(out) => devtools::cache_reply(&out.stdout).unwrap_or_else(|| {
                let err = quiet(&out.stderr);
                json!({ "error": if err.is_empty() { out.stdout.trim().chars().take(600).collect::<String>() } else { err } })
            }),
            Err(e) => {
                audit::failed("hub cache", &e);
                json!({ "error": denied(e, &mode) })
            }
        };
        message["type"] = json!("cache");
        message["request"] = json!(request.action);
        post(view, message);
        Ok(())
    }

    fn add_index(
        &mut self,
        view: &str,
        ctx: &Invocation,
        table: &str,
        column: &str,
    ) -> Result<(), Error> {
        let plain =
            |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !plain(table) || !plain(column) {
            return Err(Error::Invalid(format!(
                "not a table and column: {table}.{column}"
            )));
        }
        let (project, root) = project(ctx)?;
        let major = laravel_version(&project)
            .and_then(|v| v.split('.').next()?.parse().ok())
            .unwrap_or(11);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        let (file, content) = crate::indexadvice::migration(table, column, major, now);
        crate::renames::create_file(&project, &file, &content, "the migration")?;
        self.open_file(view, ctx, &crate::project::join(&root, &file), 1)?;
        post(
            view,
            json!({ "type": "indexadded", "table": table, "column": column, "file": file }),
        );
        Ok(())
    }

    fn runtime(&mut self, view: &str, ctx: &Invocation) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let sail = project.read_file("vendor/bin/sail").is_ok();
        let compose = runner::compose_file(&project).is_some();
        let args = mode.php_args(&["-r".into(), crate::runtime::PROBE.into()]);
        let result = crate::runner::run(&project, &mode, &args, None);
        let reply = result
            .as_ref()
            .ok()
            .and_then(|out| devtools::marked_reply(&out.stdout, crate::runtime::MARKER));
        let message = match (reply, result) {
            (Some(v), _) => {
                let binary = v[0].as_str().unwrap_or_default();
                json!({
                    "type": "runtime",
                    "ok": true,
                    "label": crate::runtime::label(&mode, binary, sail),
                    "version": v[1],
                    "os": v[2],
                    "binary": binary,
                    "ini": v[3],
                    "runner": mode.label(),
                })
            }
            (None, Err(Error::Denied(cap))) => json!({
                "type": "runtime",
                "ok": false,
                "runner": mode.label(),
                "advice": format!("Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to run PHP."),
            }),
            (None, other) => {
                if let Err(e) = &other {
                    audit::failed("hub runtime", e);
                }
                json!({
                    "type": "runtime",
                    "ok": false,
                    "runner": mode.label(),
                    "advice": crate::runtime::advice(&mode, compose, sail),
                })
            }
        };
        post(view, message);
        Ok(())
    }

    fn send_request(
        &mut self,
        view: &str,
        ctx: &Invocation,
        input: &serde_json::Value,
    ) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let label = format!(
            "{} {}",
            input["method"].as_str().unwrap_or("GET"),
            input["uri"].as_str().unwrap_or("/")
        );
        let task = host::begin_task(&format!("Sending {label}"), true);
        let args = mode.php_args(&["-r".into(), devtools::script(devtools::REQUEST_PHP)]);
        let stdin = input.to_string();
        let result = crate::runner::run(&project, &mode, &args, Some(&stdin));
        drop(task);
        let mut message = match result {
            Ok(out) => devtools::marked_reply(&out.stdout, "@@openidea-request@@").unwrap_or_else(|| {
                let err = quiet(&out.stderr);
                json!({ "error": if err.is_empty() { out.stdout.trim().chars().take(600).collect::<String>() } else { err } })
            }),
            Err(e) => {
                audit::failed("hub send request", &e);
                json!({ "error": denied(e, &mode) })
            }
        };
        message["type"] = json!("response");
        message["request"] = json!(label);
        post(view, message);
        Ok(())
    }

    fn generate_test(
        &mut self,
        view: &str,
        ctx: &Invocation,
        method: &str,
        uri: &str,
        name: &str,
        action: &str,
    ) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        let (file, pest, auth) =
            self.write_route_test(&project, &root, method, uri, name, action)?;
        self.open_file(view, ctx, &crate::project::join(&root, &file), 1)?;
        post(
            view,
            json!({ "type": "testcreated", "file": file, "pest": pest, "auth": auth }),
        );
        Ok(())
    }

    pub(crate) fn write_route_test(
        &mut self,
        project: &Project,
        root: &str,
        method: &str,
        uri: &str,
        name: &str,
        action: &str,
    ) -> Result<(String, bool, bool), Error> {
        let test = self.route_test(project, root, method, uri, name, action);
        crate::renames::create_file(project, &test.file, &test.content, "the test")?;
        Ok((test.file, test.pest, test.auth))
    }

    pub(crate) fn route_test(
        &mut self,
        project: &Project,
        root: &str,
        method: &str,
        uri: &str,
        name: &str,
        action: &str,
    ) -> RouteTest {
        let scan = self.cached_scan(project, root);
        let info = crate::routetools::info(&scan, method, uri, name, action);
        let composer = project.read_file("composer.json").unwrap_or_default();
        let pest = composer.contains("\"pestphp/pest\"");
        let user_model = project
            .read_file("app/Models/User.php")
            .is_ok()
            .then_some("User");
        let has_factory = |model: &str| {
            project
                .read_file(&format!("database/factories/{model}Factory.php"))
                .is_ok()
        };
        let mode = runner::detect(project);
        let task = host::begin_task("php artisan route:list --json", true);
        let argv: Vec<String> = ["route:list", "--json"].map(str::to_owned).to_vec();
        let listed = crate::runner::run(project, &mode, &mode.args(&argv, false), None);
        drop(task);
        let auth = listed
            .ok()
            .and_then(|out| {
                crate::routetools::middleware_of(&out.stdout, &info.methods[0], &info.uri)
            })
            .is_some_and(|m| crate::routetools::needs_auth(&m));
        let (file, content) = crate::routetools::test_file(
            &info,
            &crate::routetools::TestOptions {
                pest,
                auth,
                user_model,
                has_factory: &has_factory,
            },
        );
        RouteTest {
            file,
            content,
            pest,
            auth,
        }
    }

    fn unused_section(&mut self, project: &Project, root: &str) -> Table {
        let _task = host::begin_task("Laravel: looking for unused code", false);
        let used = crate::unused::collect(project, root, crate::string_refs);
        let routes = self.cache.get(RefKind::Route, project, root);
        let views = self.cache.get(RefKind::View, project, root);
        let translations = self.cache.get(RefKind::Translation, project, root);
        let config = self.cache.get(RefKind::Config, project, root);
        let folio = project
            .read_file("composer.json")
            .is_ok_and(|c| c.contains("\"laravel/folio\""));
        let mut rows = crate::unused::rows(
            root,
            &crate::unused::Indexes {
                routes: &routes,
                views: &views,
                translations: &translations,
                config: &config,
                folio,
            },
            &used,
        );
        let scan = self.cached_scan(project, root);
        if let Some(events) = sections::build("events", &scan, project, root, &mut self.cache) {
            for r in events.rows {
                if r.cells.get(1).map(String::as_str) == Some("app")
                    && r.cells.get(2).map(String::as_str) == Some("0")
                {
                    let place = r
                        .open
                        .as_ref()
                        .map(|t| format!("{}:{}", t.path, t.line))
                        .unwrap_or_default();
                    rows.push(Row {
                        cells: vec![
                            "Event".into(),
                            r.cells[0].clone(),
                            place,
                            "No listener handles it and it isn't broadcast".into(),
                        ],
                        open: r.open.clone(),
                    });
                }
            }
        }
        let jobs: Vec<(String, String, u32)> = inspect::jobs(project, root)
            .into_iter()
            .filter(|f| f.cells.get(1).map(String::as_str) == Some("job"))
            .map(|f| (f.cells[0].clone(), f.path, f.line))
            .collect();
        let store = self.store(root);
        rows.extend(crate::unused::class_rows(
            root,
            "Job",
            &jobs,
            |name, path| {
                let own = rel(root, path).replace('\\', "/");
                !store.files_using(name, &own).is_empty()
            },
            "No other file names the class, so nothing dispatches it",
        ));
        Table {
            columns: crate::unused::COLUMNS.to_vec(),
            rows,
            facets: vec![facet("Kind")],
        }
    }

    fn security_routes(&mut self, view: &str, ctx: &Invocation) -> Result<(), Error> {
        let (project, _) = project(ctx)?;
        let mode = runner::detect(&project);
        let task = host::begin_task("php artisan route:list --json", true);
        let argv: Vec<String> = ["route:list", "--json"].map(str::to_owned).to_vec();
        let result = crate::runner::run(&project, &mode, &mode.args(&argv, false), None);
        drop(task);
        let message = match result {
            Ok(out) => match crate::securityscan::open_routes(&out.stdout) {
                Ok(rows) => json!({ "type": "securityroutes", "rows": rows }),
                Err(e) => {
                    let err = quiet(&out.stderr);
                    json!({ "type": "securityroutes", "error": if err.is_empty() { e } else { format!("{e}: {err}") } })
                }
            },
            Err(e) => {
                audit::failed("hub security routes", &e);
                json!({ "type": "securityroutes", "error": denied(e, &mode) })
            }
        };
        post(view, message);
        Ok(())
    }

    fn diagram(&mut self, project: &Project, root: &str) -> serde_json::Value {
        let schema = crate::index::database::schema(project, root);
        let table_of: std::collections::BTreeMap<String, String> = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .filter_map(|e| {
                let table = e.detail.as_deref()?.strip_prefix("table ")?.to_owned();
                Some((e.key.clone(), table))
            })
            .collect();
        let relations: Vec<(String, String)> = self
            .cache
            .get(RefKind::Relation, project, root)
            .iter()
            .filter_map(|e| Some((e.key.clone(), e.detail.clone()?)))
            .collect();
        crate::schemadiff::diagram(root, &schema, &table_of, &relations)
    }

    fn schema_request(
        &mut self,
        view: &str,
        ctx: &Invocation,
        request: &SchemaRequest,
    ) -> Result<(), Error> {
        let script_action = match request.action {
            "compare" => "live",
            "rows" | "explain" => request.action,
            other => return Err(Error::Invalid(format!("unknown schema action {other}"))),
        };
        let (project, root) = project(ctx)?;
        let mode = runner::detect(&project);
        let task = host::begin_task("Reading the database", true);
        let args = mode.php_args(&["-r".into(), devtools::script(devtools::SCHEMA_PHP)]);
        let input = serde_json::to_string(&SchemaRequest {
            action: script_action,
            ..*request
        })
        .map_err(|e| Error::Failed(e.to_string()))?;
        let result = crate::runner::run(&project, &mode, &args, Some(&input));
        drop(task);
        let mut message = match result {
            Ok(out) => devtools::schema_reply(&out.stdout).unwrap_or_else(|| {
                let err = quiet(&out.stderr);
                json!({ "error": if err.is_empty() { out.stdout.trim().chars().take(600).collect::<String>() } else { err } })
            }),
            Err(e) => {
                audit::failed("hub schema", &e);
                json!({ "error": denied(e, &mode) })
            }
        };
        if request.action == "compare" && message.get("error").is_none() {
            let live: crate::schemadiff::Live =
                serde_json::from_value(message.clone()).unwrap_or_default();
            let schema = crate::index::database::schema(&project, &root);
            let rows = crate::schemadiff::compare(&root, &schema, &live);
            let created = schema.tables.iter().filter(|t| t.created.is_some()).count();
            let differing = rows
                .iter()
                .filter(|r| r.cells[0] != "—" && r.cells[1] != "Only in the database")
                .count();
            message["columns"] = json!(crate::schemadiff::COLUMNS);
            message["rows"] = json!(rows);
            message["synced"] = json!(created.saturating_sub(differing));
            message["tables"] = json!(live.tables.len());
        }
        message["type"] = json!("schema");
        message["request"] = json!(request.action);
        if !request.sql.is_empty() {
            message["sql"] = json!(request.sql);
        }
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
        let result = crate::runner::run(&project, &mode, &args, stdin);
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
                let (records, dropped) = devtools::fit_records(records, MAX_MESSAGE);
                post(
                    view,
                    json!({ "type": "audit", "size": text.len(), "reset": reset, "records": records, "dropped": dropped }),
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
        if action == "agents-status" {
            self.sync_mcp(&project);
        }
        let mode = runner::detect(&project);
        let (command, args) = mode.mcp_server(&root, runner::compose_file(&project));
        let access = crate::mcp::access_setting();
        let payload = crate::mcp::payload(&command, &args, access);
        let task = host::begin_task("AI agents", true);
        let run = mode.php_args(&[
            "-r".into(),
            devtools::script(devtools::INSTALL_PHP),
            action.into(),
        ]);
        let result = crate::runner::run(
            &project,
            &mode,
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
                last["access"] = json!(access);
                last
            }
            Err(e) => {
                json!({ "type": "agents", "action": action, "ok": false, "error": denied(e, &mode), "access": access })
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

impl Laravel {
    fn insert_code(&mut self, ctx: &Invocation, code: &str) -> Result<String, Error> {
        let editor = ctx
            .editor()
            .ok_or_else(|| Error::Invalid("Open a file in the editor first.".into()))?;
        let doc = editor.document()?;
        let selections = editor.selections()?;
        let first = selections
            .first()
            .ok_or_else(|| Error::Invalid("Place the cursor in the editor first.".into()))?;
        let indent = line_indent(&doc.text, first.start.line);
        let snippet = indent_snippet(code, &indent);
        editor.transform_selections(|_| snippet.clone())?;
        let path = editor.path();
        Ok(path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned())
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
            "laravel" => Ok(View::message("The hub is shown by the plugin's page.")),
            other => Err(Error::NotFound(format!("view {other}"))),
        }
    }

    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        if let Some(root) = ctx.project().and_then(|p| p.root().ok()) {
            self.status_for(&root);
        }
        let result = self.handle_request(view, event, ctx);
        if let Err(e) = &result {
            audit::failed(&format!("hub request on {view}"), e);
        }
        result
    }
}

impl Laravel {
    fn handle_request(
        &mut self,
        view: &str,
        event: ViewEvent,
        ctx: &Invocation,
    ) -> Result<(), Error> {
        let ViewEvent::Message(text) = event else {
            return Ok(());
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::Invalid(e.to_string()))?;
        let instance = value["instance"].as_str().map(str::to_owned);
        let request: Request =
            serde_json::from_value(value).map_err(|e| Error::Invalid(e.to_string()))?;
        crate::hubtabs::with_instance(instance, || self.dispatch(view, ctx, request))
    }

    fn hub_hello(&mut self, view: &str, ctx: &Invocation) -> Result<(), Error> {
        if view != crate::hubtabs::EDITOR {
            return Ok(());
        }
        let Some(path) = ctx.editor().map(|e| e.path()) else {
            return Ok(());
        };
        let Some(section) = crate::hubtabs::section_of(&path) else {
            return Ok(());
        };
        let search = self.take_hub_filter(section);
        post(
            view,
            crate::hubtabs::navigate(section, &path, search.as_deref()),
        );
        Ok(())
    }

    pub(crate) fn cached_scan(&mut self, project: &Project, root: &str) -> Rc<resources::Scan> {
        self.read_project(project, root, crate::VIEW_SLICE);
        self.store(root).scan()
    }

    fn model_overview(&mut self, view: &str, ctx: &Invocation, name: &str) -> Result<(), Error> {
        use crate::modelmap::{self, Group, Row};
        let (project, root) = project(ctx)?;
        let _task = host::begin_task(&format!("Laravel: everything about {name}"), false);
        let models = self.cache.get(RefKind::Model, &project, &root);
        let Some(entry) = models.iter().find(|e| e.key == name).cloned() else {
            post(
                view,
                json!({ "type": "error", "message": format!("{name} is not a model of this project") }),
            );
            return Ok(());
        };
        let table = entry
            .detail
            .as_deref()
            .and_then(|d| d.strip_prefix("table "))
            .unwrap_or_default()
            .to_owned();
        let model_path = rel(&root, &entry.location.path).to_owned();
        let model_text = project.read_file(&model_path).unwrap_or_default();
        let mut groups: Vec<Group> = vec![Group {
            title: "Model".into(),
            rows: vec![Row {
                label: name.to_owned(),
                detail: format!("table {table}"),
                path: model_path.clone(),
                line: entry.location.range.start.line + 1,
            }],
        }];
        let migrations: Vec<(String, String)> =
            crate::project::find(&project, &root, "database/migrations/**/*.php", 5000)
                .into_iter()
                .filter_map(|p| {
                    let r = rel(&root, &p).to_owned();
                    project.read_file(&r).ok().map(|t| (r, t))
                })
                .collect();
        let found = modelmap::migrations(&migrations, &table);
        if !found.is_empty() {
            groups.push(Group {
                title: "Migrations".into(),
                rows: found,
            });
        }
        let from_entries = |kind: RefKind, title: &str, this: &mut Self| -> Option<Group> {
            let prefix = format!("{name}.");
            let rows: Vec<Row> = this
                .cache
                .get(kind, &project, &root)
                .iter()
                .filter_map(|e| {
                    let key = e.key.strip_prefix(&prefix)?;
                    Some(Row {
                        label: key.to_owned(),
                        detail: e.detail.clone().unwrap_or_default(),
                        path: rel(&root, &e.location.path).to_owned(),
                        line: e.location.range.start.line + 1,
                    })
                })
                .collect();
            (!rows.is_empty()).then(|| Group {
                title: title.into(),
                rows,
            })
        };
        groups.extend(from_entries(RefKind::Relation, "Relations", self));
        groups.extend(from_entries(RefKind::Scope, "Scopes", self));
        let composer = project.read_file("composer.json").unwrap_or_default();
        let casts: Vec<Row> =
            crate::enums::of_model(&project, &crate::casts::psr4(&composer), &model_text)
                .into_iter()
                .map(|e| Row {
                    label: format!("{} → {}", e.column, e.name),
                    detail: e
                        .values
                        .iter()
                        .map(|(_, v)| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    path: model_path.clone(),
                    line: modelmap::first_offset(&model_text, &e.name)
                        .map_or(1, |at| modelmap::line_of(&model_text, at)),
                })
                .collect();
        if !casts.is_empty() {
            groups.push(Group {
                title: "Casts".into(),
                rows: casts,
            });
        }
        let factory = crate::factories::file(&project, name, &model_text).map(|(path, text)| Row {
            label: path
                .rsplit('/')
                .next()
                .unwrap_or(&path)
                .trim_end_matches(".php")
                .to_owned(),
            detail: "factory".into(),
            line: modelmap::line_of(&text, crate::factories::class_offset(&text)),
            path,
        });
        let scan = self.cached_scan(&project, &root);
        let usages: std::collections::BTreeMap<String, u32> = self
            .store(&root)
            .files_using(name, &model_path)
            .into_iter()
            .collect();
        let mut used = modelmap::from_scan(&scan, name, &usages);
        if let Some(row) = factory {
            match used.iter_mut().find(|g| g.title == "Factories") {
                Some(g) if !g.rows.iter().any(|r| r.path == row.path) => g.rows.insert(0, row),
                Some(_) => {}
                None => used.push(Group {
                    title: "Factories".into(),
                    rows: vec![row],
                }),
            }
        }
        groups.extend(used);
        for (dir, title) in [("routes/", "Routes"), ("tests/", "Tests")] {
            let mut rows = Vec::new();
            for r in usages.keys().filter(|p| p.starts_with(dir)) {
                if let Ok(text) = project.read_file(r) {
                    rows.extend(modelmap::lines_mentioning(r, &text, name));
                }
            }
            rows.truncate(200);
            if rows.is_empty() {
                continue;
            }
            match groups.iter_mut().find(|g| g.title == title) {
                Some(g) => g.rows.extend(rows),
                None => groups.push(Group {
                    title: title.into(),
                    rows,
                }),
            }
        }
        modelmap::sort(&mut groups);
        post(
            view,
            json!({ "type": "modelresources", "name": name, "table": table, "groups": groups }),
        );
        Ok(())
    }

    fn open_hub_tab(&mut self, view: &str, ctx: &Invocation, id: &str) -> Result<(), Error> {
        let (project, root) = project(ctx)?;
        if id != crate::hubtabs::WHOLE && !crate::hubtabs::SECTIONS.contains(&id) {
            return Err(Error::Invalid(format!("unknown hub section {id}")));
        }
        if let Err(why) = self.sync_hub_files(&project, true) {
            post(
                view,
                json!({ "type": "error", "message": format!("The hub tab files couldn't be written to storage/openidea/hub: {why}") }),
            );
            return Ok(());
        }
        let path = crate::project::join(&root, &crate::hubtabs::file_path(id));
        if let Err(e) = host::open_location(&path, 0, 0) {
            post(view, json!({ "type": "error", "message": e.to_string() }));
        }
        Ok(())
    }

    fn dispatch(&mut self, view: &str, ctx: &Invocation, request: Request) -> Result<(), Error> {
        match request {
            Request::Overview => self.overview(view, ctx),
            Request::Runtime => self.runtime(view, ctx),
            Request::Routeinfo {
                method,
                uri,
                name,
                action,
            } => {
                let (project, root) = project(ctx)?;
                let scan = self.cached_scan(&project, &root);
                let info = crate::routetools::info(&scan, &method, &uri, &name, &action);
                let body = crate::routetools::sample_body(&info);
                post(
                    view,
                    json!({ "type": "routeinfo", "info": info, "body": body }),
                );
                Ok(())
            }
            Request::Sendrequest {
                method,
                uri,
                query,
                body,
                user,
                json: as_json,
                rollback,
            } => {
                let input = json!({
                    "method": method,
                    "uri": uri,
                    "query": query,
                    "body": body,
                    "user": if user.trim().is_empty() { serde_json::Value::Null } else { json!(user.trim()) },
                    "json": as_json,
                    "rollback": rollback,
                });
                self.send_request(view, ctx, &input)
            }
            Request::Gentest {
                method,
                uri,
                name,
                action,
            } => {
                let result = self.generate_test(view, ctx, &method, &uri, &name, &action);
                if let Err(e) = &result {
                    post(view, json!({ "type": "error", "message": e.to_string() }));
                }
                result
            }
            Request::Copy { text } => {
                let message = match host::copy_to_clipboard(&text) {
                    Ok(()) => {
                        json!({ "type": "copied", "ok": true, "message": "Copied to the clipboard." })
                    }
                    Err(Error::Denied(cap)) => {
                        json!({ "type": "copied", "ok": false, "message": format!("Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to copy from the hub.") })
                    }
                    Err(e) => json!({ "type": "copied", "ok": false, "message": e.to_string() }),
                };
                post(view, message);
                Ok(())
            }
            Request::Indexcheck { sqls } => {
                let (project, root) = project(ctx)?;
                let schema = crate::index::database::schema(&project, &root);
                let results: serde_json::Map<String, serde_json::Value> = sqls
                    .iter()
                    .take(200)
                    .map(|sql| {
                        let advice = crate::indexadvice::advise(&schema, sql).map(|mut a| {
                            a.migration = a.migration.map(|m| rel(&root, &m).to_owned());
                            a
                        });
                        (sql.clone(), json!(advice))
                    })
                    .collect();
                post(view, json!({ "type": "indexcheck", "results": results }));
                Ok(())
            }
            Request::Addindex { table, column } => {
                let result = self.add_index(view, ctx, &table, &column);
                if let Err(e) = &result {
                    post(view, json!({ "type": "error", "message": e.to_string() }));
                }
                result
            }
            Request::Securityroutes => self.security_routes(view, ctx),
            Request::Diagram => {
                let (project, root) = project(ctx)?;
                let message = self.diagram(&project, &root);
                post(view, message);
                Ok(())
            }
            Request::Hello => self.hub_hello(view, ctx),
            Request::Search { query } => self.hub_search(view, ctx, &query),
            Request::Model { name } => self.model_overview(view, ctx, &name),
            Request::Openeditor { id } => self.open_hub_tab(view, ctx, &id),
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
                host::storage::set(TINKER_KEY, &tabs.to_string()).or_warn("saving the Tinker tabs");
                Ok(())
            }
            Request::Tinkerload => {
                let tabs: serde_json::Value = host::storage::get(TINKER_KEY)
                    .or_warn("reading the Tinker tabs")
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).or_warn("parsing the Tinker tabs"))
                    .unwrap_or(serde_json::Value::Null);
                post(view, json!({ "type": "tinkertabs", "tabs": tabs }));
                Ok(())
            }
            Request::Audit { action } => self.audit(view, ctx, &action),
            Request::Audittail { size } => self.audit_tail(view, ctx, size),
            Request::Agents { action } => self.agents(view, ctx, &action),
            Request::Composer { argv } => self.composer(view, ctx, argv),
            Request::Cache {
                action,
                store,
                page,
                size,
                search,
                key,
            } => self.cache_store(
                view,
                ctx,
                &CacheRequest {
                    action: &action,
                    store: &store,
                    page,
                    size,
                    search: &search,
                    key: &key,
                },
            ),
            Request::Schema {
                action,
                table,
                page,
                size,
                sql,
                bindings,
                connection,
            } => self.schema_request(
                view,
                ctx,
                &SchemaRequest {
                    action: &action,
                    table: &table,
                    page,
                    size,
                    sql: &sql,
                    bindings: &bindings,
                    connection: &connection,
                },
            ),
            Request::Sqlload => {
                let (project, root) = project(ctx)?;
                let models: Vec<serde_json::Value> = self
                    .cache
                    .get(RefKind::Model, &project, &root)
                    .iter()
                    .filter_map(|e| {
                        let table = e.detail.as_deref()?.strip_prefix("table ")?;
                        Some(json!({ "name": e.key, "table": table }))
                    })
                    .collect();
                let state: serde_json::Value = host::storage::get(SQL_KEY)
                    .or_warn("reading the SQL to Laravel state")
                    .flatten()
                    .and_then(|s| {
                        serde_json::from_str(&s).or_warn("parsing the SQL to Laravel state")
                    })
                    .unwrap_or(serde_json::Value::Null);
                post(
                    view,
                    json!({ "type": "sqlstate", "state": state, "models": models }),
                );
                Ok(())
            }
            Request::Sqlsave { state } => {
                host::storage::set(SQL_KEY, &state.to_string())
                    .or_warn("saving the SQL to Laravel state");
                Ok(())
            }
            Request::Sqlcopy { code } => {
                let message = match host::copy_to_clipboard(&code) {
                    Ok(()) => {
                        json!({ "type": "sqlresult", "ok": true, "message": "Copied to the clipboard." })
                    }
                    Err(Error::Denied(cap)) => {
                        json!({ "type": "sqlresult", "ok": false, "message": format!("Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to copy from the hub.") })
                    }
                    Err(e) => json!({ "type": "sqlresult", "ok": false, "message": e.to_string() }),
                };
                post(view, message);
                Ok(())
            }
            Request::Php2sql { code, seq } => {
                let project = project(ctx).ok();
                let result = self.php_to_sql(project.as_ref().map(|(p, r)| (p, r.as_str())), &code);
                let message = match result {
                    Ok(out) => {
                        json!({ "type": "php2sql", "seq": seq, "sql": out.sql, "notes": out.notes })
                    }
                    Err(error) => json!({ "type": "php2sql", "seq": seq, "error": error }),
                };
                post(view, message);
                Ok(())
            }
            Request::Sqlinsert { code } => {
                let message = match self.insert_code(ctx, &code) {
                    Ok(path) => {
                        json!({ "type": "sqlresult", "ok": true, "message": format!("Inserted into {path}.") })
                    }
                    Err(Error::Denied(cap)) => {
                        json!({ "type": "sqlresult", "ok": false, "message": format!("Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to insert into the editor.") })
                    }
                    Err(e) => json!({ "type": "sqlresult", "ok": false, "message": e.to_string() }),
                };
                post(view, message);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_laravel_code_to_mysql_for_the_hub() {
        use openidea_plugin::views::{ViewEvent, Views as _};
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file("artisan", "#!/usr/bin/env php\n");
        let mut plugin = Laravel::default();
        let message = json!({ "type": "php2sql", "seq": 7, "code": "DB::table('users')->where('active', 1)->count();" }).to_string();
        plugin
            .handle("laravel", ViewEvent::Message(message), &host.invocation())
            .unwrap();
        let reply: serde_json::Value =
            serde_json::from_str(&host.view_messages().last().unwrap().1).unwrap();
        assert_eq!(reply["seq"], 7);
        assert_eq!(
            reply["sql"],
            "SELECT COUNT(*) AS `aggregate`\nFROM `users`\nWHERE `active` = 1"
        );
        let bad = json!({ "type": "php2sql", "seq": 8, "code": "$user->posts" }).to_string();
        plugin
            .handle("laravel", ViewEvent::Message(bad), &host.invocation())
            .unwrap();
        let reply: serde_json::Value =
            serde_json::from_str(&host.view_messages().last().unwrap().1).unwrap();
        assert!(reply["error"].as_str().unwrap().contains("Start the chain"));
    }

    #[test]
    fn inserts_converted_sql_at_the_cursor() {
        use openidea_plugin::views::{ViewEvent, Views as _};
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file("artisan", "#!/usr/bin/env php\n");
        host.add_file(
            "composer.json",
            "{\"require\": {\"laravel/framework\": \"^12.0\"}}",
        );
        host.open_editor(
            "/project/app/Http/Controllers/A.php",
            "php",
            "<?php\nclass A {\n    public function index()\n    {\n        \n    }\n}\n",
            &[(4, 8, 4, 8)],
        );
        let mut plugin = Laravel::default();
        let message =
            json!({ "type": "sqlinsert", "code": "$users = DB::table('users')\n    ->get();\n" })
                .to_string();
        plugin
            .handle(
                "laravel",
                ViewEvent::Message(message.clone()),
                &host.invocation(),
            )
            .unwrap();
        assert!(
            host.editor_text()
                .contains("        $users = DB::table('users')\n            ->get();\n    }"),
            "{}",
            host.editor_text()
        );
        assert!(
            host.view_messages()
                .iter()
                .any(|(_, m)| m.contains("Inserted into A.php"))
        );

        let host = openidea_plugin::mock::MockHost::install()
            .with_capabilities(&["project.read", "editor.read"]);
        host.add_file("artisan", "#!/usr/bin/env php\n");
        host.open_editor("/project/app/A.php", "php", "<?php\n", &[(1, 0, 1, 0)]);
        Laravel::default()
            .handle("laravel", ViewEvent::Message(message), &host.invocation())
            .unwrap();
        assert!(
            host.view_messages()
                .iter()
                .any(|(_, m)| m.contains("editor.write")),
            "{:?}",
            host.view_messages()
        );
        assert_eq!(host.editor_text(), "<?php\n");
    }

    #[test]
    fn indents_inserted_code_like_the_line() {
        let code = "$users = DB::table('users')\n    ->where('a', 1)\n\n    ->get();\n";
        assert_eq!(
            indent_snippet(code, "        "),
            "$users = DB::table('users')\n            ->where('a', 1)\n\n            ->get();"
        );
        assert_eq!(line_indent("<?php\n    \treturn;\n", 1), "    \t");
    }

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
    }

    #[test]
    fn hub_icons_ship_with_the_page() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
        let hub = std::fs::read_to_string(dir.join("hub.html")).unwrap();
        let start = hub.find("const ICON_FILES = {").unwrap();
        let end = start + hub[start..].find("};").unwrap();
        let files: Vec<&str> = hub[start..end]
            .split('"')
            .filter(|s| s.ends_with(".svg"))
            .collect();
        assert!(files.contains(&"laravel.svg"), "{files:?}");
        for file in &files {
            let svg = std::fs::read_to_string(dir.join("icons").join(file)).unwrap();
            assert!(svg.trim_start().starts_with("<svg"), "{file}");
        }
        assert!(!hub.contains("__ICONS__"));
    }
}
