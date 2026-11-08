mod artisan;
mod audit;
mod blade_syntax;
mod casts;
mod checks;
mod closures;
mod completions;
mod configfiles;
mod devtools;
mod directives;
mod dotenv;
mod eloquent;
mod enums;
mod factories;
mod formrequest;
mod framework;
mod helpers;
mod hierarchy;
mod hubtabs;
mod index;
mod inspect;
mod inspections;
mod logs;
mod mcp;
mod members;
mod modeldoc;
mod modelmap;
mod namespaces;
mod objects;
mod panels;
mod pest;
mod php;
mod project;
mod projectfiles;
mod queries;
mod querysql;
mod refs;
mod relations;
use relations::Shape;
mod relsnips;
mod renames;
mod resources;
mod runner;
mod sections;
mod snippets;
mod statics;
mod toolconfig;
mod tools;
mod usages;

use audit::Logged;
use index::{Cache, Entry, RefKind, is_model, parse_class};
use openidea_plugin::providers::{CodeAction, CompletionItem, Document, LineMarker, Providers};
use openidea_plugin::text::{offset, position};
use openidea_plugin::{
    Context, Diagnostic, Error, Event, EventKind, Invocation, Location, Position, Project, Range,
    Severity, StatusItem, TextEdit, host,
};
use php::{StringArg, is_blade};
use project::{Lines, join, project_of};
use refs::{Scope, Segment, Want};
use std::collections::{BTreeMap, BTreeSet};

const MAX_COMPLETIONS: usize = 500;
const STATUS_ID: &str = "laravel";
const USAGE_FILES: usize = 300;
const USAGE_TOOLTIP_LINES: usize = 12;
const LARAVEL_MARKER: &str = "icons/laravel.svg";
const MANY_CHANGES: usize = 200;
const WATCHED: [&str; 14] = [
    "app/**",
    "routes/**",
    "config/**",
    "database/**",
    "resources/**",
    "lang/**",
    "tests/**",
    "bootstrap/app.php",
    "bootstrap/providers.php",
    "bootstrap/cache/routes-v7.php",
    "composer.lock",
    "vendor/composer/installed.json",
    ".env",
    ".env.example",
];

fn scope_name(method: &resources::Method) -> Option<String> {
    if let Some(rest) = method.name.strip_prefix("scope")
        && rest.chars().next().is_some_and(char::is_uppercase)
    {
        let mut chars = rest.chars();
        let first = chars.next()?;
        return Some(first.to_lowercase().chain(chars).collect());
    }
    method
        .attributes
        .iter()
        .any(|a| resources::short(&a.name) == "Scope")
        .then(|| method.name.clone())
}

fn usage_marker(root: &str, line: u32, name: &str, used: &[(String, u32)]) -> Option<LineMarker> {
    let (path, first) = used.first()?;
    let files = if used.len() == 1 { "file" } else { "files" };
    let mut tooltip = format!("{name} is used in {} {files}", used.len());
    for (path, line) in used.iter().take(USAGE_TOOLTIP_LINES) {
        tooltip.push_str(&format!("\n{path}:{line}"));
    }
    if used.len() > USAGE_TOOLTIP_LINES {
        tooltip.push_str("\n…");
    }
    if used.len() > 1 {
        tooltip.push_str("\nCtrl+click the name to choose one");
    }
    let at = Position::new(first.saturating_sub(1), 0);
    Some(LineMarker {
        line,
        icon: Some(LARAVEL_MARKER.into()),
        tooltip,
        target: Some(Location {
            path: join(root, path),
            range: Range { start: at, end: at },
        }),
    })
}

type References = Vec<(Segment, StringArg)>;

type HubTables = std::rc::Rc<Vec<(&'static str, sections::Table)>>;

#[derive(Default)]
pub struct Laravel {
    cache: Cache,
    projects: BTreeMap<String, bool>,
    commands: BTreeMap<String, std::rc::Rc<Vec<artisan::Command>>>,
    saved: Vec<renames::Saved>,
    originals: BTreeMap<String, String>,
    analysis: BTreeMap<String, Vec<Diagnostic>>,
    warned: BTreeSet<String>,
    pest: BTreeMap<String, pest::Names>,
    mcp_synced: BTreeMap<String, String>,
    helpers_synced: BTreeMap<String, String>,
    listed: BTreeMap<String, (u64, Vec<String>)>,
    hub_pending: Option<hubtabs::Pending>,
    hub_synced: BTreeSet<String>,
    hub_tables: Option<(String, HubTables)>,
    facts_cache: Option<(String, std::rc::Rc<checks::Parts>)>,
    live: BTreeMap<String, u64>,
    files: BTreeMap<String, projectfiles::Store>,
}

const EVENT_SLICE: std::time::Duration = std::time::Duration::from_millis(250);
const PROVIDER_SLICE: std::time::Duration = std::time::Duration::from_millis(60);
pub(crate) const VIEW_SLICE: std::time::Duration = std::time::Duration::from_millis(2500);

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn studly(name: &str) -> String {
    name.split(['-', '_'])
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn related_candidates(rel: &str) -> Vec<String> {
    let class_to_view = |prefix: &str, views: &str| {
        rel.strip_prefix(prefix)
            .and_then(|r| r.strip_suffix(".php"))
            .map(|r| {
                let parts: Vec<String> = r.split('/').map(kebab).collect();
                format!("{views}{}.blade.php", parts.join("/"))
            })
    };
    let view_to_class = |views: &str, prefix: &str| {
        rel.strip_prefix(views)
            .and_then(|r| r.strip_suffix(".blade.php"))
            .map(|r| {
                let parts: Vec<String> = r.split('/').map(studly).collect();
                format!("{prefix}{}.php", parts.join("/"))
            })
    };
    let mut out = Vec::new();
    out.extend(class_to_view("app/Livewire/", "resources/views/livewire/"));
    out.extend(class_to_view(
        "app/Http/Livewire/",
        "resources/views/livewire/",
    ));
    out.extend(class_to_view(
        "app/View/Components/",
        "resources/views/components/",
    ));
    out.extend(view_to_class("resources/views/livewire/", "app/Livewire/"));
    out.extend(view_to_class(
        "resources/views/livewire/",
        "app/Http/Livewire/",
    ));
    out.extend(view_to_class(
        "resources/views/components/",
        "app/View/Components/",
    ));
    if let Some(r) = rel
        .strip_prefix("resources/views/livewire/")
        .and_then(|r| r.strip_suffix("/index.blade.php"))
    {
        let parts: Vec<String> = r.split('/').map(studly).collect();
        out.push(format!("app/Livewire/{}.php", parts.join("/")));
    }
    out
}

fn scope_of(text: &str) -> Scope {
    match parse_class(text) {
        Some(class) => Scope {
            model: is_model(&class),
            class: Some(class.name),
        },
        None => Scope::default(),
    }
}

fn word_start(text: &str) -> usize {
    text.char_indices()
        .rev()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
        .map_or(0, |(i, c)| i + c.len_utf8())
}

fn pivot_access(text: &str, cursor: usize) -> Option<(String, usize)> {
    let before = text.get(..cursor)?;
    let start = word_start(before);
    let head = before[..start].strip_suffix("->")?;
    let head = head.strip_suffix('?').unwrap_or(head);
    let accessor_start = word_start(head);
    let accessor = &head[accessor_start..];
    let lead = &head[..accessor_start];
    let lead = lead.strip_suffix("->")?;
    let lead = lead.strip_suffix('?').unwrap_or(lead);
    let valid = !accessor.is_empty()
        && !accessor.starts_with(|c: char| c.is_ascii_digit())
        && lead.ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ')' || c == ']');
    valid.then(|| (accessor.to_owned(), start))
}

fn is_env_file(path: &str) -> bool {
    path.rsplit(['/', '\\'])
        .next()
        .is_some_and(|name| name == ".env" || name.starts_with(".env."))
}

fn schema_properties(doc: &Document, call_start: usize) -> Vec<Entry> {
    let tokens = php::tokenize(&doc.text, 0, doc.text.len());
    let Some(open) = tokens.iter().position(|t| t.start == call_start) else {
        return Vec::new();
    };
    let mut depth = 0usize;
    let mut end = doc.text.len();
    for t in &tokens[open..] {
        match t.kind {
            php::Kind::Open(_) => depth += 1,
            php::Kind::Close(_) => {
                depth -= 1;
                if depth == 0 {
                    end = t.start;
                    break;
                }
            }
            _ => {}
        }
    }
    let lines = Lines::new(&doc.text);
    php::string_args(&tokens)
        .into_iter()
        .filter(|a| {
            a.start > call_start
                && a.end < end
                && a.callee.name == "property"
                && a.param.as_deref() == Some("property")
        })
        .map(|a| {
            Entry::new(
                a.value,
                Some("property".into()),
                &doc.path,
                lines.range(&doc.text, a.start, a.end),
            )
        })
        .collect()
}

fn path_params(doc: &Document, before: usize) -> Vec<Entry> {
    let lines = Lines::new(&doc.text);
    let Some(path) = php::args_in(&doc.text, false)
        .into_iter()
        .rfind(|a| a.start < before && a.param.as_deref() == Some("path"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut rest = path.value.as_str();
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let name = rest[open + 1..open + close].trim_end_matches('?');
        out.push(Entry::new(
            name,
            Some(path.value.clone()),
            &doc.path,
            lines.range(&doc.text, path.start, path.end),
        ));
        rest = &rest[open + close..];
    }
    out
}

fn object_items(members: &objects::Members, typed: &str, replace: Range) -> Vec<CompletionItem> {
    let lower = typed.to_lowercase();
    let mut out = Vec::new();
    let mut field = |name: &str, detail: String, rank: &str| {
        if name.to_lowercase().contains(&lower) {
            out.push(CompletionItem {
                detail: Some(detail),
                replace: Some(replace),
                sort_text: Some(format!("{rank}{name}")),
                ..CompletionItem::new(name, openidea_plugin::providers::CompletionKind::Field)
            });
        }
    };
    if members.kind == "enum" {
        field(
            "name",
            format!("string · case name of {}", members.name),
            "0",
        );
        if let Some(backed) = &members.backed {
            field("value", format!("{backed} · backed value"), "0");
        }
    }
    for (name, ty) in &members.properties {
        let detail = if ty.is_empty() {
            members.name.clone()
        } else {
            ty.clone()
        };
        field(name, detail, "1");
    }
    let methods: Vec<(&str, &str)> = members
        .methods
        .iter()
        .filter(|(_, _, is_static)| !is_static)
        .map(|(n, s, _)| (n.as_str(), s.as_str()))
        .collect();
    let mut calls = relations::method_items(&methods, &members.name, typed, replace);
    for item in &mut calls {
        item.sort_text = item.sort_text.take().map(|s| format!("2{s}"));
    }
    out.extend(calls);
    out
}

fn relation_target(detail: Option<&str>) -> Option<String> {
    let d = detail?;
    Some(d[d.find('(')? + 1..d.find(')')?].to_owned())
}

fn tag_at(
    text: &str,
    cursor: usize,
    prefixes: &[(&str, RefKind)],
) -> Option<(RefKind, usize, usize)> {
    let name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':');
    let start = text[..cursor]
        .rfind(|c: char| !name_char(c))
        .map_or(0, |i| i + 1);
    let end = text[cursor..]
        .find(|c: char| !name_char(c))
        .map_or(text.len(), |i| cursor + i);
    let lt = text[..start].rfind('<')?;
    let tag = &text[lt..end];
    for (prefix, kind) in prefixes {
        let name_start = lt + prefix.len();
        if tag.starts_with(prefix)
            && !text[lt..start].contains(char::is_whitespace)
            && name_start <= cursor
        {
            return Some((*kind, name_start, end));
        }
    }
    None
}

const TAGS: [(&str, RefKind); 2] = [
    ("<x-", RefKind::Component),
    ("<livewire:", RefKind::Livewire),
];

impl Laravel {
    fn project_symbols(&mut self, root: &str) -> std::rc::Rc<Vec<hierarchy::ProjectSymbol>> {
        self.store(root).symbols()
    }

    pub(crate) fn store(&mut self, root: &str) -> &mut projectfiles::Store {
        self.files.entry(root.to_owned()).or_default()
    }

    pub(crate) fn read_project(
        &mut self,
        project: &Project,
        root: &str,
        budget: std::time::Duration,
    ) -> bool {
        let store = self.store(root);
        store.ready() || store.advance(project, root, budget)
    }

    fn usages_ready(&self, root: &str) -> bool {
        self.files.get(root).is_some_and(projectfiles::Store::ready)
    }

    fn laravel_project(&mut self, path: &str) -> Option<(Project, String)> {
        let project = project_of(path)?;
        let root = project.root().ok()?;
        let laravel = match self.projects.get(&root) {
            Some(known) => *known,
            None => {
                let known = project::is_laravel(&project);
                self.projects.insert(root.clone(), known);
                if known {
                    show_status(&project);
                }
                known
            }
        };
        laravel.then_some((project, root))
    }

    fn index_in_background(&mut self, path: &str) {
        if let Some((project, root)) = self.laravel_project(path)
            && (!self.cache.has_framework(&root)
                || !self.facts_ready(&root)
                || !self.usages_ready(&root))
        {
            let _task = host::begin_task("Indexing the Laravel project", false);
            if !self.cache.has_framework(&root) || !self.facts_ready(&root) {
                self.cache.warm(&project, &root);
                self.cached_facts(&project, &root);
            }
            self.read_project(&project, &root, EVENT_SLICE);
        }
    }

    fn forget_everything(&mut self) {
        self.hub_tables = None;
        self.facts_cache = None;
        self.files.clear();
        self.originals.clear();
        self.live.clear();
        self.cache.clear();
    }

    fn absolute(&self, path: &str) -> String {
        if runner::is_windows_root(path) || path.starts_with('/') {
            return path.to_owned();
        }
        self.projects
            .iter()
            .filter(|(_, laravel)| **laravel)
            .map(|(root, _)| join(root, path))
            .find(|p| project_of(p).is_some())
            .unwrap_or_else(|| path.to_owned())
    }

    fn disk_changed(&mut self, path: &str) -> bool {
        let Some(project) = project_of(path) else {
            return false;
        };
        let Ok(root) = project.root() else {
            return false;
        };
        self.hub_tables = None;
        self.facts_cache = None;
        self.live.remove(path);
        self.reindex_usages(&project, &root, path);
        self.cache.invalidate(&root, path);
        if path.replace('\\', "/").contains("/tests/") {
            self.pest.remove(&root);
        }
        match project.read_file(path) {
            Ok(text) => {
                self.remember_original(&root, path, &text, true);
                true
            }
            Err(_) => {
                self.originals.remove(path);
                false
            }
        }
    }

    fn reindex_usages(&mut self, project: &Project, root: &str, path: &str) {
        if let Some(store) = self.files.get_mut(root) {
            store.update(project, root, path);
        }
    }

    fn publish_live(&mut self, doc: &Document) {
        if !doc.path.ends_with(".php") || is_blade(&doc.path) {
            return;
        }
        if self.live.get(&doc.path) == Some(&doc.version) {
            return;
        }
        let Some((_, root)) = self.laravel_project(&doc.path) else {
            return;
        };
        if !self.facts_ready(&root) {
            return;
        }
        self.live.insert(doc.path.clone(), doc.version);
        let mut items = self.inspect(&doc.path, &doc.text);
        if let Some(found) = self.analysis.get(&doc.path) {
            items.extend(found.iter().cloned());
        }
        host::publish_diagnostics(&doc.path, &items)
            .or_warn(&format!("publishing diagnostics for {}", doc.path));
    }

    fn model_table(&mut self, project: &Project, root: &str, model: &str) -> Option<String> {
        let models = self.cache.get(RefKind::Model, project, root);
        let entry = models.iter().find(|e| e.key == model)?;
        entry
            .detail
            .as_deref()?
            .strip_prefix("table ")
            .map(str::to_owned)
    }

    fn member_items(
        &mut self,
        project: &Project,
        root: &str,
        class: &str,
        text: &str,
        ctx: &members::Context,
        at: Position,
    ) -> Vec<CompletionItem> {
        let table = self.model_table(project, root, class).unwrap_or_default();
        let column_entries = self.cache.get(RefKind::Column, project, root);
        let models: Vec<(String, String)> = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .filter_map(|e| {
                Some((
                    e.key.clone(),
                    e.detail.as_deref()?.strip_prefix("table ")?.to_owned(),
                ))
            })
            .collect();
        let foreign: Vec<(String, String)> = column_entries
            .iter()
            .filter_map(|e| {
                let (t, c) = e.key.split_once('.')?;
                c.ends_with("_id").then(|| (t.to_owned(), c.to_owned()))
            })
            .collect();
        let prefix = format!("{class}.");
        let relations: Vec<String> = self
            .cache
            .get(RefKind::Relation, project, root)
            .iter()
            .filter_map(|e| e.key.strip_prefix(&prefix).map(str::to_owned))
            .collect();
        let framework = self.cache.framework(root);
        let model = members::Model {
            table: &table,
            columns: members::columns_of(&column_entries, &table),
            models,
            foreign,
            relations,
            casts_method: !framework.installed || framework.casts_method,
        };
        members::items(&model, ctx, at, position(text, ctx.start))
    }

    pub(crate) fn env_key_entries(&mut self, project: &Project, root: &str) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .cache
            .get(RefKind::Config, project, root)
            .iter()
            .filter_map(|e| {
                let key = dotenv::env_key(e.detail.as_deref()?)?;
                Some(Entry {
                    key: key.to_owned(),
                    detail: Some(e.key.clone()),
                    location: e.location.clone(),
                })
            })
            .collect();
        entries.extend(dotenv::LARAVEL_KEYS.iter().map(|(k, d)| {
            Entry::new(
                *k,
                (!d.is_empty()).then(|| (*d).to_owned()),
                "",
                Range::default(),
            )
        }));
        entries.extend(self.cache.get(RefKind::Env, project, root).iter().cloned());
        entries
    }

    pub(crate) fn env_value_entries(
        &mut self,
        project: &Project,
        root: &str,
        key: &str,
    ) -> Option<(RefKind, Vec<Entry>)> {
        let fixed = |values: &[(&str, &str)]| -> Vec<Entry> {
            values
                .iter()
                .map(|(v, d)| {
                    Entry::new(
                        *v,
                        (!d.is_empty()).then(|| (*d).to_owned()),
                        "",
                        Range::default(),
                    )
                })
                .collect()
        };
        Some(match dotenv::values_for(key)? {
            dotenv::Values::Kind(kind) => (kind, self.cache.get(kind, project, root).to_vec()),
            dotenv::Values::Static(values) => (RefKind::Rule, fixed(values)),
            dotenv::Values::Boolean => (RefKind::Rule, fixed(&[("true", ""), ("false", "")])),
            dotenv::Values::Locales => {
                let mut locales: BTreeSet<String> = self
                    .cache
                    .get(RefKind::Translation, project, root)
                    .iter()
                    .filter_map(|e| {
                        e.detail
                            .as_deref()
                            .map(|d| d.split(':').next().unwrap_or(d).trim().to_owned())
                    })
                    .collect();
                locales.extend(["en", "en_US", "pt_BR", "es", "fr", "de", "it"].map(str::to_owned));
                (
                    RefKind::Rule,
                    locales
                        .into_iter()
                        .map(|l| Entry::new(l, Some("locale".into()), "", Range::default()))
                        .collect(),
                )
            }
        })
    }

    fn env_completion(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
        at: Position,
    ) -> Vec<CompletionItem> {
        match dotenv::context(&doc.text, cursor) {
            Some(dotenv::Context::Key { start, typed }) => {
                let present: BTreeSet<String> = doc
                    .text
                    .lines()
                    .filter_map(|l| {
                        let l = l.trim_start();
                        let l = l.strip_prefix("export ").unwrap_or(l);
                        l.split_once('=').map(|(k, _)| k.trim().to_owned())
                    })
                    .collect();
                let mut entries = self.env_key_entries(project, root);
                entries.retain(|e| !present.contains(&e.key));
                items(
                    RefKind::Env,
                    &entries,
                    &typed,
                    Range::new(position(&doc.text, start), at),
                )
                .into_iter()
                .map(|mut i| {
                    i.insert_text = format!("{}=", i.label);
                    i
                })
                .collect()
            }
            Some(dotenv::Context::Value { key, start, typed }) => {
                let Some((kind, entries)) = self.env_value_entries(project, root, &key) else {
                    return Vec::new();
                };
                items(
                    kind,
                    &entries,
                    &typed,
                    Range::new(position(&doc.text, start), at),
                )
            }
            None => Vec::new(),
        }
    }

    fn route_targets(
        &mut self,
        project: &Project,
        root: &str,
        text: &str,
        cursor: usize,
    ) -> Vec<Location> {
        let controllers = self.cache.get(RefKind::Controller, project, root);
        index::parse_routes(text)
            .into_iter()
            .filter(|r| {
                (r.start..=r.end).contains(&cursor)
                    || r.uri.is_some_and(|(s, e)| (s..=e).contains(&cursor))
            })
            .filter_map(|r| r.action)
            .filter_map(|(class, method)| {
                let key = format!("{class}.{method}");
                controllers
                    .iter()
                    .find(|e| e.key == key)
                    .map(|e| e.location.clone())
            })
            .collect()
    }

    fn model_of(
        &mut self,
        project: &Project,
        root: &str,
        at: (&str, usize),
        base: &eloquent::Base,
        path: &[String],
        depth: u8,
    ) -> Option<(String, bool)> {
        match self.chain_of(project, root, at, base, path, depth)? {
            (model, Shape::Model) => Some((model, false)),
            (model, Shape::Many) => Some((model, true)),
            (_, Shape::Query | Shape::Factory) => None,
        }
    }

    fn chain_of(
        &mut self,
        project: &Project,
        root: &str,
        (text, cursor): (&str, usize),
        base: &eloquent::Base,
        path: &[String],
        depth: u8,
    ) -> Option<(String, Shape)> {
        if depth > 4 {
            return None;
        }
        let models: BTreeSet<String> = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .map(|e| e.key.clone())
            .collect();
        let (mut current, mut shape) = match base {
            eloquent::Base::This => {
                let scope = scope_of(text);
                (scope.class.filter(|_| scope.model)?, Shape::Model)
            }
            eloquent::Base::AuthUser => ("User".to_owned(), Shape::Model),
            eloquent::Base::Static(class) => {
                let class = if matches!(class.as_str(), "static" | "self") {
                    scope_of(text).class?
                } else {
                    class.clone()
                };
                (class, Shape::Query)
            }
            eloquent::Base::Var(var) => match eloquent::origin(text, cursor, var) {
                Some(eloquent::Origin::Class { name, many }) => {
                    (name, if many { Shape::Many } else { Shape::Model })
                }
                Some(eloquent::Origin::Chain {
                    base,
                    path,
                    element,
                }) => {
                    let (model, many) =
                        self.model_of(project, root, (text, cursor), &base, &path, depth + 1)?;
                    if element && !many {
                        return None;
                    }
                    (
                        model,
                        if many && !element {
                            Shape::Many
                        } else {
                            Shape::Model
                        },
                    )
                }
                None => {
                    let param =
                        eloquent::closure_param(text, cursor, var).filter(|p| !p.builder)?;
                    (
                        self.closure_model(project, root, text, &param, depth + 1)?,
                        Shape::Model,
                    )
                }
            },
        };
        if !models.contains(&current) {
            return None;
        }
        let relations = self.cache.get(RefKind::Relation, project, root);
        let scopes = self.cache.get(RefKind::Scope, project, root);
        for step in path {
            if let Some(call) = step.strip_suffix("()") {
                let relation = (shape == Shape::Model)
                    .then(|| {
                        relations
                            .iter()
                            .find(|e| e.key == format!("{current}.{call}"))
                            .and_then(|e| relation_target(e.detail.as_deref()))
                    })
                    .flatten();
                if let Some(target) = relation {
                    current = target;
                    shape = Shape::Query;
                    continue;
                }
                if shape != Shape::Many
                    && scopes.iter().any(|e| e.key == format!("{current}.{call}"))
                {
                    shape = Shape::Query;
                    continue;
                }
                shape = relations::after_call(shape, call)?;
                continue;
            }
            if shape != Shape::Model {
                return None;
            }
            let entry = relations
                .iter()
                .find(|e| e.key == format!("{current}.{step}"))?;
            let detail = entry.detail.as_deref()?;
            shape = if eloquent::is_many(detail.split('(').next().unwrap_or(detail)) {
                Shape::Many
            } else {
                Shape::Model
            };
            current = relation_target(Some(detail))?;
        }
        Some((current, shape))
    }

    fn head_model(
        &mut self,
        project: &Project,
        root: &str,
        (text, cursor): (&str, usize),
        head: &str,
        steps: &[String],
        depth: u8,
    ) -> Option<String> {
        if depth > 4 {
            return None;
        }
        let mut current = match head.strip_prefix('$') {
            Some("this") => {
                let scope = scope_of(text);
                scope.class.filter(|_| scope.model)?
            }
            Some(var) => match self.model_of(
                project,
                root,
                (text, cursor),
                &eloquent::Base::Var(var.to_owned()),
                &[],
                depth + 1,
            ) {
                Some((model, _)) => model,
                None => {
                    let param = eloquent::closure_param(text, cursor, var)?;
                    self.closure_model(project, root, text, &param, depth + 1)?
                }
            },
            None => head.to_owned(),
        };
        let models = self.cache.get(RefKind::Model, project, root);
        if !models.iter().any(|e| e.key == current) {
            return None;
        }
        let relations = self.cache.get(RefKind::Relation, project, root);
        for step in steps {
            if let Some(entry) = relations
                .iter()
                .find(|e| e.key == format!("{current}.{step}"))
                && let Some(target) = relation_target(entry.detail.as_deref())
            {
                current = target;
            }
        }
        Some(current)
    }

    fn closure_model(
        &mut self,
        project: &Project,
        root: &str,
        text: &str,
        param: &eloquent::ClosureParam,
        depth: u8,
    ) -> Option<String> {
        let mut model = self.head_model(
            project,
            root,
            (text, param.at),
            &param.head,
            &param.steps,
            depth,
        )?;
        if let Some(related) = &param.related {
            let relations = self.cache.get(RefKind::Relation, project, root);
            for step in related.split('.') {
                let entry = relations
                    .iter()
                    .find(|e| e.key == format!("{model}.{step}"))?;
                model = relation_target(entry.detail.as_deref())?;
            }
        }
        Some(model)
    }

    fn infer_head(&mut self, project: &Project, root: &str, text: &str, arg: &mut php::StringArg) {
        let tokens = php::tokenize(text, 0, (arg.call_start + 1).min(text.len()));
        let Some(open) = tokens
            .iter()
            .rposition(|t| t.start == arg.call_start && t.kind == php::Kind::Open('('))
        else {
            return;
        };
        let Some((head, steps)) = php::chain(&tokens, open) else {
            return;
        };
        if !head.starts_with('$') && steps.is_empty() || head == "$this" {
            return;
        }
        if let Some(model) =
            self.head_model(project, root, (text, arg.call_start), &head, &steps, 0)
        {
            arg.head = Some(model);
        }
    }

    fn model_text(&mut self, project: &Project, root: &str, model: &str) -> Option<String> {
        let path = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .find(|e| e.key == model)?
            .location
            .path
            .clone();
        crate::project::read_listed(project, &path).ok()
    }

    fn eloquent_members(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
        at: Position,
    ) -> Option<Vec<CompletionItem>> {
        let access = eloquent::access_at(&doc.text, cursor, false)?;
        let typed = &doc.text[access.start..cursor];
        let replace = Range::new(position(&doc.text, access.start), at);
        if let Some((last, parents)) = access.path.split_last()
            && let Some(name) = last.strip_suffix("()")
            && let Some((parent, false)) =
                self.model_of(project, root, (&doc.text, cursor), &access.base, parents, 0)
            && let Some(detail) = self
                .cache
                .get(RefKind::Relation, project, root)
                .iter()
                .find(|e| e.key == format!("{parent}.{name}"))
                .and_then(|e| e.detail.clone())
            && let Some(target) = relation_target(Some(&detail))
        {
            let kind = detail.split('(').next().unwrap_or(&detail);
            let framework = self.cache.framework(root);
            return Some(relations::method_items(
                &relations::relation_methods(kind, &framework),
                &target,
                typed,
                replace,
            ));
        }
        if let Some((column, parents)) = access.path.split_last()
            && !column.ends_with("()")
            && let Some((model, false)) =
                self.model_of(project, root, (&doc.text, cursor), &access.base, parents, 0)
            && let Some(model_text) = self.model_text(project, root, &model)
            && let Some((path, text)) = casts::target_file(project, &model_text, column)
            && let Some(members) = objects::with_traits(project, (&path, &text), 0)
        {
            return Some(object_items(&members, typed, replace));
        }
        let (model, shape) = self.chain_of(
            project,
            root,
            (&doc.text, cursor),
            &access.base,
            &access.path,
            0,
        )?;
        match shape {
            Shape::Many => {
                let framework = self.cache.framework(root);
                return Some(relations::method_items(
                    &relations::collection_methods(&framework),
                    &model,
                    typed,
                    replace,
                ));
            }
            Shape::Query => {
                let prefix = format!("{model}.");
                let scopes: Vec<(String, &str)> = self
                    .cache
                    .get(RefKind::Scope, project, root)
                    .iter()
                    .filter_map(|e| e.key.strip_prefix(&prefix))
                    .map(|name| (name.to_owned(), "scope: Builder<{T}>"))
                    .collect();
                let mut out = relations::method_items(&scopes, &model, typed, replace);
                for item in &mut out {
                    item.sort_text = item.sort_text.take().map(|s| format!("!{s}"));
                }
                let framework = self.cache.framework(root);
                out.extend(relations::method_items(
                    &relations::query_methods(&framework),
                    &model,
                    typed,
                    replace,
                ));
                if access.path.is_empty()
                    && matches!(access.base, eloquent::Base::Static(_))
                    && let Some(model_text) = self.model_text(project, root, &model)
                    && factories::file(project, &model, &model_text).is_some()
                {
                    let factory = [("factory", "($count = null, $state = []): Factory<{T}>")];
                    out.extend(relations::method_items(&factory, &model, typed, replace));
                }
                return Some(out);
            }
            Shape::Factory => {
                let framework = self.cache.framework(root);
                let model_text = self.model_text(project, root, &model).unwrap_or_default();
                let states = factories::file(project, &model, &model_text)
                    .map(|(_, text)| factories::states(&text))
                    .unwrap_or_default();
                let mut out = relations::method_items(&states, &model, typed, replace);
                for item in &mut out {
                    item.sort_text = item.sort_text.take().map(|s| format!("!{s}"));
                }
                out.extend(relations::method_items(
                    &relations::factory_methods(&framework),
                    &model,
                    typed,
                    replace,
                ));
                return Some(out);
            }
            Shape::Model => {}
        }
        let mut out = Vec::new();
        if let Some(table) = self.model_table(project, root, &model) {
            let model_text = self.model_text(project, root, &model).unwrap_or_default();
            let columns: Vec<Entry> = self
                .prefixed(RefKind::Column, project, root, &table)
                .into_iter()
                .map(|e| {
                    let cast = casts::resolve(project, root, &model_text, &e.key);
                    match (cast, e.detail.as_deref()) {
                        (Some(c), Some(d)) => Entry {
                            detail: Some(format!("{d} → {}", c.label)),
                            ..e
                        },
                        (Some(c), None) => Entry {
                            detail: Some(c.label),
                            ..e
                        },
                        (None, _) => e,
                    }
                })
                .collect();
            out.extend(items(RefKind::Column, &columns, typed, replace));
        }
        let relations: Vec<Entry> = self
            .prefixed(RefKind::Relation, project, root, &model)
            .into_iter()
            .map(|e| {
                let detail = e.detail.as_deref().map(|d| {
                    let kind = d.split('(').next().unwrap_or(d);
                    match relation_target(Some(d)) {
                        Some(target) => format!("{kind} → {target}"),
                        None => kind.to_owned(),
                    }
                });
                Entry { detail, ..e }
            })
            .collect();
        out.extend(
            items(RefKind::Model, &relations, typed, replace)
                .into_iter()
                .map(|mut i| {
                    i.sort_text = i.sort_text.map(|s| format!("~{s}"));
                    i
                }),
        );
        Some(out)
    }

    fn model_fqn(&mut self, project: &Project, root: &str, model: &str) -> Option<String> {
        let path = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .find(|e| e.key == model)?
            .location
            .path
            .clone();
        let composer = project.read_file("composer.json").unwrap_or_default();
        let namespace = namespaces::for_path(project::rel(root, &path), &casts::psr4(&composer))
            .into_iter()
            .next()?;
        Some(format!("{namespace}\\{model}"))
    }

    fn static_candidates(&mut self, project: &Project, root: &str) -> Vec<statics::Candidate> {
        let framework = self.cache.framework(root);
        let mut facades = framework.facades.clone();
        if facades.is_empty() {
            facades =
                statics::config_aliases(&project.read_file("config/app.php").unwrap_or_default());
        }
        let symbols = self.project_symbols(root);
        let mut out: Vec<statics::Candidate> = symbols
            .iter()
            .filter(|s| s.statically || s.cast)
            .map(|s| statics::Candidate {
                fqn: s.fqn.clone(),
                detail: match s.keyword.as_str() {
                    "enum" => "enum".to_owned(),
                    _ if s.statically => "static class".to_owned(),
                    _ => "cast".to_owned(),
                },
                cast: s.cast,
                statically: s.statically,
            })
            .collect();
        out.extend(facades.into_iter().map(|(alias, fqn)| statics::Candidate {
            detail: if fqn.contains("\\Facades\\") {
                "Laravel facade".to_owned()
            } else {
                format!("Laravel {alias}")
            },
            fqn,
            cast: false,
            statically: true,
        }));
        out
    }

    fn model_of_file(&mut self, project: &Project, root: &str, path: &str) -> Option<String> {
        let path = path.replace('\\', "/");
        self.cache
            .get(RefKind::Model, project, root)
            .iter()
            .find(|e| {
                e.location
                    .path
                    .replace('\\', "/")
                    .eq_ignore_ascii_case(&path)
            })
            .map(|e| e.key.clone())
    }

    fn scope_usages(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        model: &str,
        scope: &str,
    ) -> Vec<(String, u32)> {
        let own = project::rel(root, &doc.path).replace('\\', "/");
        let candidates = self.store(root).files_using(scope, "");
        let mut out = Vec::new();
        for (path, _) in candidates.into_iter().take(USAGE_FILES) {
            let text = if path == own {
                doc.text.clone()
            } else {
                match project.read_file(&project::join(root, &path)) {
                    Ok(text) => text,
                    Err(_) => continue,
                }
            };
            let cleaned = usages::clean(&text, path.ends_with(".blade.php"));
            if let Some((start, _)) = eloquent::scope_calls(&cleaned, model, scope).first() {
                out.push((path, usages::line_at(&cleaned, *start) + 1));
            }
        }
        out
    }

    fn usage_markers(&mut self, doc: &Document, project: &Project, root: &str) -> Vec<LineMarker> {
        let own = project::rel(root, &doc.path).replace('\\', "/");
        if !(own.starts_with("app/") || own.starts_with("database/"))
            || is_blade(&doc.path)
            || !self.read_project(project, root, PROVIDER_SLICE)
        {
            return Vec::new();
        }
        let scan = resources::scan_text(&own, &doc.text);
        let model = self.model_of_file(project, root, &doc.path);
        let mut out = Vec::new();
        for symbol in &scan.symbols {
            let used = self.store(root).files_using(&symbol.name, &own);
            out.extend(usage_marker(
                root,
                symbol.line.saturating_sub(1),
                &symbol.name,
                &used,
            ));
            if model.as_deref() != Some(symbol.name.as_str()) {
                continue;
            }
            for method in &symbol.methods {
                let Some(scope) = scope_name(method) else {
                    continue;
                };
                let used = self.scope_usages(project, root, doc, &symbol.name, &scope);
                out.extend(usage_marker(
                    root,
                    method.line.saturating_sub(1),
                    &format!("scope {scope}()"),
                    &used,
                ));
            }
        }
        out
    }

    fn declaration_usages(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<Vec<Location>> {
        let own = project::rel(root, &doc.path).replace('\\', "/");
        if !(own.starts_with("app/") || own.starts_with("database/")) || is_blade(&doc.path) {
            return None;
        }
        let text = &doc.text;
        let (start, end) = eloquent::word_span(text, cursor)?;
        let before = text[..start].trim_end();
        let keyword = before
            .rsplit(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or_default();
        if !matches!(keyword, "class" | "enum" | "interface" | "trait") {
            return None;
        }
        let name = &text[start..end];
        if !self.usages_ready(root) {
            return None;
        }
        let files = self.store(root).files_using(name, &own);
        let mut out = Vec::new();
        for (path, _) in files.into_iter().take(USAGE_FILES) {
            let full = project::join(root, &path);
            let Ok(source) = project.read_file(&full) else {
                continue;
            };
            let cleaned = usages::clean(&source, path.ends_with(".blade.php"));
            let lines = Lines::new(&source);
            for at in usages::occurrences(&cleaned, name) {
                out.push(Location {
                    path: full.clone(),
                    range: lines.range(&source, at, at + name.len()),
                });
            }
        }
        (!out.is_empty()).then_some(out)
    }

    fn relation_snippets(
        &mut self,
        project: &Project,
        root: &str,
        class: &str,
        text: &str,
        cursor: usize,
        at: Position,
    ) -> Vec<CompletionItem> {
        let Some(start) = snippets::word_at(text, cursor) else {
            return Vec::new();
        };
        let table = self.model_table(project, root, class).unwrap_or_default();
        let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for e in self.cache.get(RefKind::Column, project, root).iter() {
            if let Some((t, c)) = e.key.split_once('.') {
                tables.entry(t.to_owned()).or_default().push(c.to_owned());
            }
        }
        let composer = project.read_file("composer.json").unwrap_or_default();
        let map = casts::psr4(&composer);
        let mut models: Vec<(String, String)> = Vec::new();
        let mut fqns: BTreeMap<String, String> = BTreeMap::new();
        for e in self.cache.get(RefKind::Model, project, root).iter() {
            if let Some(t) = e.detail.as_deref().and_then(|d| d.strip_prefix("table ")) {
                models.push((e.key.clone(), t.to_owned()));
            }
            if let Some(ns) = namespaces::for_path(project::rel(root, &e.location.path), &map)
                .into_iter()
                .next()
            {
                fqns.insert(e.key.clone(), format!("{ns}\\{}", e.key));
            }
        }
        let prefix = format!("{class}.");
        let existing: Vec<String> = self
            .cache
            .get(RefKind::Relation, project, root)
            .iter()
            .filter_map(|e| e.key.strip_prefix(&prefix).map(str::to_owned))
            .collect();
        let namespace = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("namespace "))
            .map(|n| n.trim_end_matches(';').trim().to_owned());
        let import = |model: &str| {
            fqns.get(model)
                .filter(|fqn| fqn.rsplit_once('\\').map(|(ns, _)| ns) != namespace.as_deref())
                .cloned()
        };
        relsnips::items(
            &relsnips::Facts {
                model: class,
                table: &table,
                tables: &tables,
                models: &models,
                existing: &existing,
            },
            &relsnips::Context {
                typed: &text[start..cursor],
                replace: Range::new(position(text, start), at),
                text,
                import: &import,
            },
        )
    }

    fn closure_items(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
        at: Position,
    ) -> Option<Vec<CompletionItem>> {
        let site = closures::site(&doc.text, cursor)?;
        let found = match &site.callee {
            closures::Callee::Member { name_end, .. } => {
                eloquent::access_at(&doc.text, *name_end, false).and_then(|access| {
                    self.chain_of(
                        project,
                        root,
                        (&doc.text, cursor),
                        &access.base,
                        &access.path,
                        0,
                    )
                    .map(|(m, s)| (m, s, false))
                })
            }
            closures::Callee::Static { class, .. } => {
                let short = class.rsplit('\\').next().unwrap_or(class).to_owned();
                let base = eloquent::Base::Static(short);
                self.chain_of(project, root, (&doc.text, cursor), &base, &[], 0)
                    .map(|(m, s)| (m, s, true))
            }
            closures::Callee::Function { .. } => None,
        };
        let fqn = found
            .as_ref()
            .and_then(|(model, _, _)| self.model_fqn(project, root, model));
        let receiver = found
            .as_ref()
            .map(|(model, shape, is_static)| closures::Receiver {
                model,
                model_fqn: fqn.as_deref(),
                shape: *shape,
                is_static: *is_static,
            });
        let user_fqn = self.model_fqn(project, root, "User");
        let user = user_fqn.as_deref().map(|fqn| ("User", fqn));
        let params = closures::params(&site, receiver.as_ref(), user)?;
        let namespace = doc
            .text
            .lines()
            .find_map(|l| l.trim().strip_prefix("namespace "))
            .map(|n| n.trim_end_matches(';').trim().to_owned());
        let imports: Vec<&str> = params
            .iter()
            .filter_map(|p| p.fqn.as_deref())
            .filter(|fqn| fqn.rsplit_once('\\').map(|(ns, _)| ns) != namespace.as_deref())
            .collect();
        let edits = snippets::import_edits(&doc.text, &imports);
        let (shown, escaped) = closures::signature(&params);
        let replace = Range::new(position(&doc.text, site.start), at);
        let item = |label: String, insert: String, filter: &str, rank: &str| CompletionItem {
            detail: Some("typed closure".into()),
            insert_text: insert,
            snippet: true,
            replace: Some(replace),
            sort_text: Some(format!("!{rank}")),
            filter_text: Some(filter.to_owned()),
            additional_edits: edits.clone(),
            ..CompletionItem::new(&label, openidea_plugin::providers::CompletionKind::Snippet)
        };
        Some(vec![
            item(
                format!("fn ({shown}) =>"),
                format!("fn ({escaped}) => $0"),
                "fn",
                "0",
            ),
            item(
                format!("function ({shown})"),
                format!("function ({escaped}) {{\n\t$0\n}}"),
                "function",
                "1",
            ),
        ])
    }

    fn factory_definition(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<Vec<Location>> {
        let text = &doc.text;
        let (start, end) = eloquent::word_span(text, cursor)?;
        if &text[start..end] == "HasFactory" {
            return self.has_factory_definition(project, root, text);
        }
        if &text[start..end] != "factory" || !text[end..].trim_start().starts_with('(') {
            return None;
        }
        let class_end = text[..start].trim_end().strip_suffix("::")?;
        let class_start = class_end
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'))
            .map_or(0, |i| i + 1);
        let class = class_end[class_start..].trim_start_matches('\\');
        let model = class.rsplit('\\').next().unwrap_or(class);
        let model = if matches!(model, "static" | "self") {
            scope_of(text).class?
        } else {
            model.to_owned()
        };
        let model_text = self.model_text(project, root, &model)?;
        if let Some((path, factory)) = factories::file(project, &model, &model_text) {
            let at = factories::class_offset(&factory);
            let lines = Lines::new(&factory);
            return Some(vec![Location {
                path: project::join(root, &path),
                range: lines.range(&factory, at, at),
            }]);
        }
        let framework = self.cache.framework(root);
        Some(framework.factory_method.clone().into_iter().collect())
    }

    fn has_factory_definition(
        &mut self,
        project: &Project,
        root: &str,
        text: &str,
    ) -> Option<Vec<Location>> {
        let model = scope_of(text).class?;
        let model_text = self.model_text(project, root, &model)?;
        if let Some((path, factory)) = factories::file(project, &model, &model_text) {
            let at = factories::class_offset(&factory);
            return Some(vec![Location {
                path: project::join(root, &path),
                range: Lines::new(&factory).range(&factory, at, at),
            }]);
        }
        let method = self.cache.framework(root).factory_method.clone()?;
        let trait_file = project
            .read_file(project::rel(root, &method.path))
            .ok()
            .and_then(|source| {
                let at = modelmap::first_offset(&source, "trait HasFactory")?;
                let at = at + "trait ".len();
                Some(Location {
                    path: method.path.clone(),
                    range: Lines::new(&source).range(&source, at, at + "HasFactory".len()),
                })
            });
        Some(vec![trait_file.unwrap_or(method)])
    }

    fn scope_call_definition(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<Vec<Location>> {
        let text = &doc.text;
        let (start, end) = eloquent::word_span(text, cursor)?;
        if !text[end..].trim_start().starts_with('(') {
            return None;
        }
        let name = &text[start..end];
        let before = text[..start].trim_end();
        let model = if let Some(class_end) = before.strip_suffix("::") {
            let class = &class_end[eloquent::word_span(class_end, class_end.len())?.0..];
            Some(class.rsplit('\\').next().unwrap_or(class).to_owned())
        } else if before.ends_with("->") {
            eloquent::access_at(text, cursor, true).and_then(|a| {
                self.model_of(project, root, (text, cursor), &a.base, &a.path, 0)
                    .map(|(m, _)| m)
            })
        } else {
            return None;
        };
        let scopes = self.cache.get(RefKind::Scope, project, root);
        let of_model: Vec<Location> = model
            .iter()
            .flat_map(|m| {
                scopes
                    .iter()
                    .filter(move |e| e.key == format!("{m}.{name}"))
            })
            .map(|e| e.location.clone())
            .collect();
        let found = if of_model.is_empty() {
            scopes
                .iter()
                .filter(|e| e.key.rsplit('.').next() == Some(name))
                .map(|e| e.location.clone())
                .collect()
        } else {
            of_model
        };
        (!found.is_empty()).then_some(found)
    }

    fn eloquent_definition(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<Vec<Location>> {
        let access = eloquent::access_at(&doc.text, cursor, true)?;
        let word = doc.text[access.start..access.end].to_owned();
        let (model, many) = self.model_of(
            project,
            root,
            (&doc.text, cursor),
            &access.base,
            &access.path,
            0,
        )?;
        if many {
            return None;
        }
        let relation = self
            .cache
            .get(RefKind::Relation, project, root)
            .iter()
            .find(|e| e.key == format!("{model}.{word}"))
            .cloned();
        if let Some(relation) = relation {
            let target = relation_target(relation.detail.as_deref())?;
            let found: Vec<Location> = self
                .cache
                .get(RefKind::Model, project, root)
                .iter()
                .filter(|e| e.key == target)
                .map(|e| e.location.clone())
                .collect();
            return Some(if found.is_empty() {
                vec![relation.location]
            } else {
                found
            });
        }
        if let Some(text) = self.model_text(project, root, &model)
            && let Some(location) =
                casts::resolve(project, root, &text, &word).and_then(|t| t.location)
        {
            return Some(vec![location]);
        }
        let table = self.model_table(project, root, &model)?;
        let column = format!("{table}.{word}");
        let found: Vec<Location> = self
            .cache
            .get(RefKind::Column, project, root)
            .iter()
            .filter(|e| e.key == column && !e.location.path.is_empty())
            .map(|e| e.location.clone())
            .collect();
        (!found.is_empty()).then_some(found)
    }

    fn walk_relations(
        &mut self,
        project: &Project,
        root: &str,
        model: &str,
        path: &[String],
    ) -> Option<String> {
        let relations = self.cache.get(RefKind::Relation, project, root);
        let mut current = model.to_owned();
        for step in path {
            let entry = relations
                .iter()
                .find(|e| e.key == format!("{current}.{step}"))?;
            current = relation_target(entry.detail.as_deref())?;
        }
        Some(current)
    }

    fn prefixed(
        &mut self,
        kind: RefKind,
        project: &Project,
        root: &str,
        prefix: &str,
    ) -> Vec<Entry> {
        let prefix = format!("{prefix}.");
        self.cache
            .get(kind, project, root)
            .iter()
            .filter_map(|e| {
                let key = e.key.strip_prefix(&prefix)?;
                Some(Entry {
                    key: key.to_owned(),
                    ..e.clone()
                })
            })
            .collect()
    }

    fn resolve(
        &mut self,
        want: &Want,
        project: &Project,
        root: &str,
        doc: &Document,
    ) -> (RefKind, Vec<Entry>) {
        match want {
            Want::Kind(RefKind::Ability) => {
                let mut entries = self.cache.get(RefKind::Ability, project, root).to_vec();
                entries.extend(
                    self.cache
                        .get(RefKind::Permission, project, root)
                        .iter()
                        .cloned(),
                );
                (RefKind::Ability, entries)
            }
            Want::Kind(kind) => (*kind, self.cache.get(*kind, project, root).to_vec()),
            Want::OpenApiRef => {
                let entries = self
                    .cache
                    .get(RefKind::OpenApi, project, root)
                    .iter()
                    .filter(|e| !e.key.starts_with("tags/"))
                    .map(|e| Entry {
                        key: format!("#/components/{}", e.key),
                        ..e.clone()
                    })
                    .collect();
                (RefKind::OpenApi, entries)
            }
            Want::OpenApiSection(section) => {
                let prefix = format!("{section}/");
                let entries = self
                    .cache
                    .get(RefKind::OpenApi, project, root)
                    .iter()
                    .filter_map(|e| {
                        Some(Entry {
                            key: e.key.strip_prefix(&prefix)?.to_owned(),
                            ..e.clone()
                        })
                    })
                    .collect();
                (RefKind::OpenApi, entries)
            }
            Want::Static(values) => (RefKind::Rule, fixed_entries(values)),
            Want::Container => {
                let framework = self.cache.framework(root);
                if framework.container.is_empty() {
                    (RefKind::Rule, fixed_entries(&refs::CONTAINER))
                } else {
                    let entries = framework
                        .container
                        .iter()
                        .map(|(k, class)| Entry::new(k, Some(class.clone()), "", Range::default()))
                        .collect();
                    (RefKind::Rule, entries)
                }
            }
            Want::CastTypes => {
                let framework = self.cache.framework(root);
                if framework.casts.is_empty() {
                    return (RefKind::Rule, fixed_entries(&refs::CAST_TYPES));
                }
                let mut entries: Vec<Entry> = refs::CAST_TYPES
                    .iter()
                    .filter(|(k, _)| {
                        if k.contains('\\') {
                            framework.has_class(k)
                        } else {
                            framework.casts.iter().any(|c| c == k)
                        }
                    })
                    .map(|(k, d)| Entry::new(*k, Some((*d).to_owned()), "", Range::default()))
                    .collect();
                for cast in &framework.casts {
                    if !entries.iter().any(|e| &e.key == cast) {
                        entries.push(Entry::new(
                            cast,
                            Some("cast type".into()),
                            "",
                            Range::default(),
                        ));
                    }
                }
                (RefKind::Rule, entries)
            }
            Want::SchemaProperties(call_start) => {
                (RefKind::Column, schema_properties(doc, *call_start))
            }
            Want::PathParams(before) => (RefKind::Column, path_params(doc, *before)),
            Want::Paths(base) => (RefKind::Asset, self.path_items(project, root, base)),
            Want::DiskPaths(call_start) => {
                let config = project
                    .read_file("config/filesystems.php")
                    .unwrap_or_default();
                let disk = helpers::disk_before(&doc.text, *call_start);
                let entries = helpers::disk_root(&config, disk.as_deref())
                    .map(|base| self.path_items(project, root, &base))
                    .unwrap_or_default();
                (RefKind::Asset, entries)
            }
            Want::RequestFields => (
                RefKind::Column,
                self.request_fields(project, root, &doc.text, is_blade(&doc.path)),
            ),
            Want::RoutePaths => {
                let mut seen = BTreeSet::new();
                let mut entries = Vec::new();
                for kind in [RefKind::Route, RefKind::RouteAction] {
                    for e in self.cache.get(kind, project, root).iter() {
                        let Some((verb, rest)) =
                            e.detail.as_deref().and_then(|d| d.split_once(' '))
                        else {
                            continue;
                        };
                        let uri = rest.split(' ').next().unwrap_or(rest);
                        if uri.starts_with('/') && seen.insert(uri.to_owned()) {
                            entries.push(Entry {
                                key: uri.to_owned(),
                                detail: Some(verb.to_owned()),
                                location: e.location.clone(),
                            });
                        }
                    }
                }
                (RefKind::Route, entries)
            }
            Want::EnumValues {
                model,
                call_start,
                index,
            } => {
                let args = enums::call_args(&doc.text, *call_start);
                let column = enums::value_column(&args, *index).map(str::to_owned);
                let entries = column
                    .zip(self.model_text(project, root, model))
                    .and_then(|(column, model_text)| {
                        let composer = project.read_file("composer.json").unwrap_or_default();
                        enums::of_model(project, &casts::psr4(&composer), &model_text)
                            .into_iter()
                            .find(|e| e.column == column)
                    })
                    .map(|cast| {
                        cast.values
                            .iter()
                            .map(|(case, value)| {
                                Entry::new(
                                    value,
                                    Some(format!("{}::{case}", cast.name)),
                                    "",
                                    Range::default(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (RefKind::Env, entries)
            }
            Want::Columns { model, table } => {
                let table = table
                    .clone()
                    .or_else(|| self.model_table(project, root, model.as_deref()?));
                let entries = table
                    .map(|t| self.prefixed(RefKind::Column, project, root, &t))
                    .unwrap_or_default();
                (RefKind::Column, entries)
            }
            Want::Relations { model, path } => {
                let entries = self
                    .walk_relations(project, root, model, path)
                    .map(|m| self.prefixed(RefKind::Relation, project, root, &m))
                    .unwrap_or_default();
                (RefKind::Relation, entries)
            }
            Want::RelationColumns { model, path } => {
                let entries = self
                    .walk_relations(project, root, model, path)
                    .and_then(|m| self.model_table(project, root, &m))
                    .map(|t| self.prefixed(RefKind::Column, project, root, &t))
                    .unwrap_or_default();
                (RefKind::Column, entries)
            }
            Want::Methods(class) => (
                RefKind::Controller,
                self.prefixed(RefKind::Controller, project, root, class),
            ),
        }
    }

    fn references(&mut self, doc: &Document) -> Option<(Project, String, References)> {
        let (project, root) = self.laravel_project(&doc.path)?;
        let scope = scope_of(&doc.text);
        let refs = php::args_in(&doc.text, is_blade(&doc.path))
            .into_iter()
            .filter_map(|a| refs::segment(&a, a.value.len(), &scope).map(|s| (s, a)))
            .collect();
        Some((project, root, refs))
    }

    fn inspect(&mut self, path: &str, text: &str) -> Vec<Diagnostic> {
        let doc = Document {
            path: path.to_owned(),
            language: "php".into(),
            version: 0,
            text: text.to_owned(),
        };
        let Some((project, root, refs)) = self.references(&doc) else {
            return Vec::new();
        };
        let enabled = |kind: RefKind| match kind {
            RefKind::View => host::setting_bool("unknownViewInspection").unwrap_or(true),
            RefKind::Route => host::setting_bool("unknownRouteInspection").unwrap_or(false),
            RefKind::Middleware | RefKind::Ability => {
                host::setting_bool("unknownAuthInspection").unwrap_or(false)
            }
            RefKind::Rule => host::setting_bool("unknownRuleInspection").unwrap_or(true),
            _ => false,
        };
        let lines = Lines::new(text);
        let mut out = Vec::new();
        for (seg, arg) in refs {
            if seg.want == Want::OpenApiRef {
                let Some(component) = arg.value.strip_prefix("#/components/") else {
                    continue;
                };
                if !arg.closed || !host::setting_bool("unknownOpenApiRefInspection").unwrap_or(true)
                {
                    continue;
                }
                if self
                    .cache
                    .get(RefKind::OpenApi, &project, &root)
                    .iter()
                    .any(|e| e.key == component)
                {
                    continue;
                }
                out.push(
                    Diagnostic::new(
                        lines.range(text, arg.start, arg.end),
                        Severity::Warning,
                        format!("OpenAPI component '{}' not found", arg.value),
                    )
                    .code("laravel.unknown-openapi-ref"),
                );
                continue;
            }
            let Want::Kind(kind) = seg.want else { continue };
            let value = &arg.value[seg.start.min(arg.value.len())..seg.end.min(arg.value.len())];
            if !arg.closed || arg.concat || !enabled(kind) || !refs::checkable(kind, value) {
                continue;
            }
            if self
                .cache
                .get(kind, &project, &root)
                .iter()
                .any(|e| e.key == value)
            {
                continue;
            }
            let label = match kind {
                RefKind::View => "View",
                RefKind::Route => "Route",
                RefKind::Middleware => "Middleware",
                RefKind::Ability => "Ability",
                _ => "Validation rule",
            };
            out.push(
                Diagnostic::new(
                    lines.range(text, arg.start + seg.start, arg.start + seg.end),
                    Severity::Warning,
                    format!("{label} '{value}' not found"),
                )
                .code(format!(
                    "laravel.unknown-{}",
                    label.to_lowercase().replace(' ', "-")
                )),
            );
        }
        let found = self.findings(&project, &root, path, text);
        out.extend(Self::finding_diagnostics(text, &found));
        out
    }

    fn publish(&mut self, path: &str) {
        if self.publish_env(path) || !path.ends_with(".php") {
            return;
        }
        let Some(project) = project_of(path) else {
            return;
        };
        let Some(text) = project
            .read_file(path)
            .or_warn(&format!("reading {path} for diagnostics"))
        else {
            return;
        };
        let mut items = self.inspect(path, &text);
        if let Some(found) = self.analysis.get(path) {
            items.extend(found.iter().cloned());
        }
        host::publish_diagnostics(path, &items)
            .or_error(&format!("publishing diagnostics for {path}"));
    }

    fn artisan_terminal(&self, ctx: &Invocation, command: &str) -> Result<Option<String>, Error> {
        let project = ctx.project().ok_or("open a Laravel project first")?;
        let mode = runner::detect(&project);
        host::open_terminal(
            &format!("artisan {command}"),
            mode.program(),
            &mode.args(&[command.to_owned()], true),
        )?;
        Ok(None)
    }

    fn artisan(&self, ctx: &Invocation, args: &[&str]) -> Result<Option<String>, Error> {
        let project = ctx.project().ok_or("open a Laravel project first")?;
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let mode = runner::detect(&project);
        let out = crate::runner::run(&project, &mode, &mode.args(&args[1..], false), None)?;
        let text = if out.stdout.trim().is_empty() {
            out.stderr
        } else {
            out.stdout
        };
        if out.status != Some(0) {
            let status = out.status.map_or("a signal".to_owned(), |s| s.to_string());
            return Err(Error::Failed(format!(
                "artisan {} exited with {status}: {}",
                args[1],
                text.trim()
            )));
        }
        Ok(Some(
            text.trim().lines().last().unwrap_or("done").to_owned(),
        ))
    }

    fn controller_markers(
        &mut self,
        doc: &Document,
        project: &Project,
        root: &str,
    ) -> Vec<LineMarker> {
        if !doc.path.replace('\\', "/").contains("/Http/Controllers/") {
            return Vec::new();
        }
        let Some(class) = parse_class(&doc.text) else {
            return Vec::new();
        };
        let actions = self.cache.get(RefKind::RouteAction, project, root);
        let lines = Lines::new(&doc.text);
        class
            .methods
            .iter()
            .filter_map(|m| {
                let key = format!("{}@{}", class.name, m.name);
                let routes: Vec<&Entry> = actions.iter().filter(|e| e.key == key).collect();
                let first = routes.first()?;
                let tooltip = routes
                    .iter()
                    .filter_map(|e| e.detail.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(LineMarker {
                    line: lines.position(&doc.text, m.start).line,
                    icon: Some("implementing".into()),
                    tooltip: format!("Route: {tooltip}"),
                    target: Some(first.location.clone()),
                })
            })
            .collect()
    }

    fn model_doc_action(
        &mut self,
        doc: &Document,
        project: &Project,
        root: &str,
        cursor: usize,
    ) -> Option<CodeAction> {
        if is_blade(&doc.path) || !doc.path.ends_with(".php") {
            return None;
        }
        let class = parse_class(&doc.text)?;
        let header_start = doc.text[..class.start].rfind('\n').map_or(0, |i| i + 1);
        let header_end = doc.text[class.start..]
            .find('{')
            .map_or(doc.text.len(), |i| class.start + i);
        if cursor < header_start.saturating_sub(200) || cursor > header_end {
            return None;
        }
        if !is_model(&class) {
            return None;
        }
        let models = self.cache.get(RefKind::Model, project, root);
        let table = models
            .iter()
            .find(|e| e.key == class.name)
            .and_then(|e| e.detail.as_deref())
            .map(|d| d.trim_start_matches("table ").to_owned())
            .unwrap_or_else(|| index::table_for(&class));
        let prefix = format!("{table}.");
        let columns: Vec<(String, String)> = self
            .cache
            .get(RefKind::Column, project, root)
            .iter()
            .filter_map(|e| {
                e.key
                    .strip_prefix(&prefix)
                    .map(|c| (c.to_owned(), e.detail.clone().unwrap_or_default()))
            })
            .collect();
        let rel_prefix = format!("{}.", class.name);
        let relations: Vec<(String, String)> = self
            .cache
            .get(RefKind::Relation, project, root)
            .iter()
            .filter_map(|e| {
                e.key
                    .strip_prefix(&rel_prefix)
                    .map(|r| (r.to_owned(), e.detail.clone().unwrap_or_default()))
            })
            .collect();
        let edit = modeldoc::docblock(&doc.text, class.start, &columns, &relations)?;
        Some(CodeAction {
            title: format!(
                "Generate @property PHPDoc for {} ({} columns, {} relations)",
                class.name,
                columns.len(),
                relations.len()
            ),
            kind: Some("refactor".into()),
            preferred: false,
            data: None,
            edits: vec![(
                doc.path.clone(),
                vec![TextEdit {
                    range: Range::new(
                        position(&doc.text, edit.start),
                        position(&doc.text, edit.end),
                    ),
                    text: edit.text,
                }],
            )],
        })
    }

    fn related_file(&mut self, ctx: &Invocation) -> Result<Option<String>, Error> {
        let editor = ctx.editor().ok_or("open a file first")?;
        let path = editor.path();
        let (project, root) = self
            .laravel_project(&path)
            .ok_or("this file is not in a Laravel project")?;
        let rel = project::rel(&root, &path).replace('\\', "/");
        let candidates = related_candidates(&rel);
        for c in &candidates {
            if project.read_file(c).is_ok() {
                host::open_location(c, 0, 0)?;
                return Ok(Some(format!("Opened {c}")));
            }
        }
        if candidates.is_empty() {
            Err("no related file: open a Livewire or Blade component class or its view".into())
        } else {
            Err(format!("{} does not exist yet", candidates[0]).into())
        }
    }

    fn translation_actions(&mut self, project: &Project, root: &str, key: &str) -> Vec<CodeAction> {
        let locale = if self.cache.locale.is_empty() {
            "en".to_owned()
        } else {
            self.cache.locale.clone()
        };
        let mut out = Vec::new();
        let existing = |name: String| {
            ["lang", "resources/lang"].iter().find_map(|dir| {
                let relative = format!("{dir}/{name}");
                project
                    .read_file(&relative)
                    .ok()
                    .map(|text| (relative, text))
            })
        };
        if let Some((file, rest)) = key.split_once('.')
            && !key.contains(' ')
            && let Some((relative, text)) = existing(format!("{locale}/{file}.php"))
            && let Some(close) = text.rfind(']')
        {
            let before = text[..close].trim_end();
            let comma = if before.ends_with('[') || before.ends_with(',') {
                ""
            } else {
                ","
            };
            let insert_at = before.len();
            let escaped = rest.replace('\'', "\\'");
            let pos = position(&text, insert_at);
            out.push(CodeAction {
                title: format!("Add '{key}' to {relative}"),
                kind: Some("quickfix".into()),
                preferred: true,
                data: None,
                edits: vec![(
                    join(root, &relative),
                    vec![TextEdit {
                        range: Range::new(pos, pos),
                        text: format!("{comma}\n    '{escaped}' => '{escaped}',"),
                    }],
                )],
            });
        }
        if let Some((relative, text)) = existing(format!("{locale}.json"))
            && let Some(close) = text.rfind('}')
        {
            let before = text[..close].trim_end();
            let comma = if before.ends_with('{') { "" } else { "," };
            let escaped = key.replace('\\', "\\\\").replace('"', "\\\"");
            let pos = position(&text, before.len());
            out.push(CodeAction {
                title: format!("Add '{key}' to {relative}"),
                kind: Some("quickfix".into()),
                preferred: out.is_empty(),
                data: None,
                edits: vec![(
                    join(root, &relative),
                    vec![TextEdit {
                        range: Range::new(pos, pos),
                        text: format!("{comma}\n    \"{escaped}\": \"{escaped}\""),
                    }],
                )],
            });
        }
        out
    }
}

fn php_snippets(text: &str, cursor: usize, at: Position) -> Vec<CompletionItem> {
    let Some(start) = snippets::word_at(text, cursor) else {
        return Vec::new();
    };
    snippets::items(text, cursor, Range::new(position(text, start), at))
}

fn fixed_entries(values: &[(&str, &str)]) -> Vec<Entry> {
    values
        .iter()
        .map(|(k, d)| {
            Entry::new(
                *k,
                (!d.is_empty()).then(|| (*d).to_owned()),
                "",
                Range::default(),
            )
        })
        .collect()
}

fn show_status(project: &Project) {
    let version = project::laravel_version(project);
    let text = version
        .as_deref()
        .map_or("Laravel".to_owned(), |v| format!("Laravel {v}"));
    host::set_status_item(
        STATUS_ID,
        &StatusItem {
            text,
            tooltip: Some("Laravel OpenIdea — click to refresh the index".into()),
            command: Some("refresh-index".into()),
        },
    );
}

fn items(kind: RefKind, entries: &[Entry], typed: &str, replace: Range) -> Vec<CompletionItem> {
    let typed = typed.to_lowercase();
    let mut seen = BTreeSet::new();
    entries
        .iter()
        .filter(|e| e.key.to_lowercase().contains(&typed) && seen.insert(e.key.clone()))
        .take(MAX_COMPLETIONS)
        .map(|e| {
            let rank = if e.key.to_lowercase().starts_with(&typed) {
                '0'
            } else {
                '1'
            };
            CompletionItem {
                detail: e.detail.clone(),
                replace: Some(replace),
                sort_text: Some(format!("{rank}{}", e.key)),
                ..CompletionItem::new(&e.key, kind.completion_kind())
            }
        })
        .collect()
}

impl Providers for Laravel {
    fn completion(
        &mut self,
        doc: &Document,
        at: Position,
        _trigger: Option<&str>,
    ) -> Vec<CompletionItem> {
        let Some((project, root)) = self.laravel_project(&doc.path) else {
            return Vec::new();
        };
        let Some(cursor) = offset(&doc.text, at) else {
            return Vec::new();
        };
        if is_env_file(&doc.path) {
            return self.env_completion(&project, &root, doc, cursor, at);
        }
        if let Some(items) = self.tool_completion(&project, &root, doc, cursor) {
            return items;
        }
        if let Some((accessor, start)) = pivot_access(&doc.text, cursor) {
            let prefix = format!("{accessor}.");
            let entries: Vec<Entry> = self
                .cache
                .get(RefKind::Pivot, &project, &root)
                .iter()
                .filter_map(|e| {
                    Some(Entry {
                        key: e.key.strip_prefix(&prefix)?.to_owned(),
                        ..e.clone()
                    })
                })
                .collect();
            if !entries.is_empty() {
                return items(
                    RefKind::Column,
                    &entries,
                    &doc.text[start..cursor],
                    Range::new(position(&doc.text, start), at),
                );
            }
        }
        let blade = is_blade(&doc.path);
        if !blade
            && doc.path.ends_with(".php")
            && let Some(start) = namespaces::typed_at(&doc.text, cursor)
        {
            let composer = project.read_file("composer.json").unwrap_or_default();
            let found =
                namespaces::for_path(project::rel(&root, &doc.path), &casts::psr4(&composer));
            let replace = Range::new(position(&doc.text, start), at);
            return namespaces::items(&found, &doc.text, start, cursor, replace);
        }
        if !blade
            && let Some(found) = self.closure_items(&project, &root, doc, cursor, at)
            && !found.is_empty()
        {
            return found;
        }
        let mut static_items = Vec::new();
        if !blade
            && doc.path.ends_with(".php")
            && let Some((slot, start)) = statics::slot_at(&doc.text, cursor)
        {
            let candidates = self.static_candidates(&project, &root);
            let found = statics::items(
                slot,
                &doc.text,
                &doc.text[start..cursor],
                Range::new(position(&doc.text, start), at),
                &candidates,
            );
            if slot == statics::Slot::CastValue {
                return found;
            }
            static_items = found;
        }
        if php::open_arg_at(&doc.text, blade, cursor).is_none()
            && let Some(found) = self.eloquent_members(&project, &root, doc, cursor, at)
            && !found.is_empty()
        {
            return found;
        }
        if blade && let Some(start) = directives::typed_at(&doc.text, cursor) {
            let framework = self.cache.framework(&root);
            let mut entries =
                directives::builtin(&framework, |package| framework.has_package(package));
            entries.extend(
                self.cache
                    .get(RefKind::Directive, &project, &root)
                    .iter()
                    .cloned(),
            );
            let typed = &doc.text[start + 1..cursor];
            let replace = Range::new(position(&doc.text, start), at);
            let mut out: Vec<CompletionItem> = items(RefKind::Directive, &entries, typed, replace)
                .into_iter()
                .map(|mut item| {
                    item.label = format!("@{}", item.label);
                    item.insert_text = item.label.clone();
                    item
                })
                .collect();
            out.extend(snippets::blade_items(typed, replace, |directive| {
                entries
                    .iter()
                    .any(|e| e.key.eq_ignore_ascii_case(directive))
            }));
            return out;
        }
        if blade && let Some((kind, start, _)) = tag_at(&doc.text, cursor, &TAGS) {
            let entries = self.cache.get(kind, &project, &root);
            return items(
                kind,
                &entries,
                &doc.text[start..cursor],
                Range::new(position(&doc.text, start), at),
            );
        }
        if !blade && let Some((slot, start)) = hierarchy::slot_at(&doc.text, cursor) {
            let symbols = self.project_symbols(&root);
            let composer = project.read_file("composer.json").unwrap_or_default();
            let framework = self.cache.framework(&root);
            return hierarchy::items(
                slot,
                &doc.text,
                project::rel(&root, &doc.path),
                start,
                at,
                &hierarchy::Installed {
                    composer: &composer,
                    framework_has: &|fqn| framework.has_class(fqn),
                },
                &symbols,
            );
        }
        if !blade
            && php::open_arg_at(&doc.text, false, cursor).is_none()
            && let Some(ctx) = members::context(&doc.text, cursor)
        {
            let scope = scope_of(&doc.text);
            if let (true, Some(class)) = (scope.model, scope.class.as_deref()) {
                let mut out = self.member_items(&project, &root, class, &doc.text, &ctx, at);
                out.extend(php_snippets(&doc.text, cursor, at));
                out.extend(self.relation_snippets(&project, &root, class, &doc.text, cursor, at));
                out.extend(static_items);
                return out;
            }
        }
        let Some(mut arg) = php::open_arg_at(&doc.text, blade, cursor) else {
            return if blade {
                Vec::new()
            } else {
                let mut out = php_snippets(&doc.text, cursor, at);
                out.extend(static_items);
                out
            };
        };
        if !blade {
            self.infer_head(&project, &root, &doc.text, &mut arg);
        }
        let scope = scope_of(&doc.text);
        let within = cursor - arg.start;
        let Some(seg) = refs::segment(&arg, within, &scope) else {
            return Vec::new();
        };
        let (kind, entries) = self.resolve(&seg.want, &project, &root, doc);
        let start = (arg.start + seg.start).min(cursor);
        let quote = doc.text[..arg.start]
            .chars()
            .last()
            .filter(|c| matches!(c, '\'' | '"'));
        let segmented = matches!(
            seg.want,
            Want::Kind(RefKind::Rule | RefKind::Middleware) | Want::Columns { .. }
        );
        let end = cursor
            + doc.text[cursor..]
                .find(|c: char| {
                    Some(c) == quote || c == '\n' || c == '|' || segmented && matches!(c, ',' | ':')
                })
                .unwrap_or(0);
        items(
            kind,
            &entries,
            &doc.text[start..cursor],
            Range::new(position(&doc.text, start), position(&doc.text, end)),
        )
    }

    fn definition(&mut self, doc: &Document, at: Position) -> Vec<Location> {
        let Some((project, root)) = self.laravel_project(&doc.path) else {
            return Vec::new();
        };
        let Some(cursor) = offset(&doc.text, at) else {
            return Vec::new();
        };
        if is_env_file(&doc.path) {
            let line_start = doc.text[..cursor].rfind('\n').map_or(0, |i| i + 1);
            let line = &doc.text[line_start..];
            let line = line[..line.find('\n').unwrap_or(line.len())].trim_start();
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some((key, _)) = line.split_once('=') else {
                return Vec::new();
            };
            let key = key.trim();
            return self
                .cache
                .get(RefKind::Config, &project, &root)
                .iter()
                .filter(|e| e.detail.as_deref().and_then(dotenv::env_key) == Some(key))
                .map(|e| e.location.clone())
                .collect();
        }
        if !is_blade(&doc.path)
            && let Some(arg) = php::arg_at(&doc.text, false, cursor)
            && let Some(key) = hubtabs::cache_key(&arg)
            && project.read_file(&hubtabs::file_path("cache")).is_ok()
        {
            self.remember_hub_filter("cache", key);
            return vec![Location {
                path: project::join(&root, &hubtabs::file_path("cache")),
                range: Range::default(),
            }];
        }
        if !is_blade(&doc.path)
            && let Some(arg) = php::arg_at(&doc.text, false, cursor)
            && arg.arg == 0
            && arg.callee.kind == php::CallKind::Static
            && arg.callee.class.as_deref() == Some("gate")
            && arg.callee.name == "define"
            && !arg.value.is_empty()
        {
            return renames::usage_locations(&project, &root, doc, RefKind::Ability, &arg.value);
        }
        if !is_blade(&doc.path)
            && let Some(name) = eloquent::scope_at(&doc.text, cursor)
            && let Some(model) = scope_of(&doc.text).class
        {
            return renames::locations_matching(&project, &root, doc, |text, blade| {
                eloquent::scope_calls(&usages::clean(text, blade), &model, &name)
            });
        }
        if let Some(found) = self.declaration_usages(&project, &root, doc, cursor) {
            return found;
        }
        if !is_blade(&doc.path)
            && let Some(found) = self.factory_definition(&project, &root, doc, cursor)
        {
            return found;
        }
        if !is_blade(&doc.path)
            && let Some(found) = self.scope_call_definition(&project, &root, doc, cursor)
        {
            return found;
        }
        if project::rel(&root, &doc.path)
            .replace('\\', "/")
            .starts_with("routes/")
        {
            let targets = self.route_targets(&project, &root, &doc.text, cursor);
            if !targets.is_empty() {
                return targets;
            }
        }
        let blade = is_blade(&doc.path);
        if php::arg_at(&doc.text, blade, cursor).is_none()
            && let Some(found) = self.eloquent_definition(&project, &root, doc, cursor)
        {
            return found;
        }
        let (want, key) =
            if blade && let Some((kind, start, end)) = tag_at(&doc.text, cursor, &TAGS) {
                (Want::Kind(kind), doc.text[start..end].to_owned())
            } else {
                let Some(mut arg) = php::arg_at(&doc.text, blade, cursor) else {
                    return Vec::new();
                };
                if !blade {
                    self.infer_head(&project, &root, &doc.text, &mut arg);
                }
                let scope = scope_of(&doc.text);
                let Some(seg) = refs::segment(&arg, cursor - arg.start, &scope) else {
                    return Vec::new();
                };
                let key = arg
                    .value
                    .get(seg.start..seg.end)
                    .unwrap_or_default()
                    .to_owned();
                (seg.want, key)
            };
        let (_, entries) = self.resolve(&want, &project, &root, doc);
        entries
            .into_iter()
            .filter(|e| e.key == key && !e.location.path.is_empty())
            .map(|e| e.location)
            .collect()
    }

    fn line_markers(&mut self, doc: &Document) -> Vec<LineMarker> {
        self.publish_live(doc);
        let Some((project, root, refs)) = self.references(doc) else {
            return Vec::new();
        };
        self.remember_original(&root, &doc.path, &doc.text, false);
        let lines = Lines::new(&doc.text);
        let mut out = Vec::new();
        for (seg, arg) in refs {
            if seg.want == Want::Kind(RefKind::Route) && arg.closed && !arg.value.is_empty() {
                let routes = self.cache.get(RefKind::Route, &project, &root);
                if let Some(entry) = routes.iter().find(|e| e.key == arg.value) {
                    out.push(LineMarker {
                        line: lines.position(&doc.text, arg.start).line,
                        icon: Some("navigate".into()),
                        tooltip: format!(
                            "Go to route {} ({})",
                            entry.key,
                            entry.detail.as_deref().unwrap_or_default()
                        ),
                        target: Some(entry.location.clone()),
                    });
                }
                continue;
            }
            if seg.want != Want::Kind(RefKind::View) || !arg.closed {
                continue;
            }
            let views = self.cache.get(RefKind::View, &project, &root);
            let Some(entry) = views.iter().find(|e| e.key == arg.value) else {
                continue;
            };
            out.push(LineMarker {
                line: lines.position(&doc.text, arg.start).line,
                icon: Some("navigate".into()),
                tooltip: format!(
                    "Go to view {}",
                    entry.detail.as_deref().unwrap_or(&entry.key)
                ),
                target: Some(entry.location.clone()),
            });
        }
        out.extend(self.controller_markers(doc, &project, &root));
        out.extend(self.usage_markers(doc, &project, &root));
        out
    }

    fn resolve_code_action(
        &mut self,
        doc: &Document,
        action: CodeAction,
    ) -> Result<CodeAction, Error> {
        let title = action.title.clone();
        self.resolve_pending(doc, action)
            .inspect_err(|e| audit::failed(&format!("code action \"{title}\""), e))
    }

    fn code_actions(&mut self, doc: &Document, selection: Range) -> Vec<CodeAction> {
        self.publish_live(doc);
        let Some((project, root)) = self.laravel_project(&doc.path) else {
            return Vec::new();
        };
        let Some(cursor) = offset(&doc.text, selection.start) else {
            return Vec::new();
        };
        let mut fixes = self.fix_actions(&project, &root, doc, cursor);
        fixes.extend(self.rename_actions(&project, &root, doc, cursor));
        fixes.extend(self.form_request_action(&project, doc, cursor));
        if !fixes.is_empty() {
            return fixes;
        }
        if let Some(action) = self.model_doc_action(doc, &project, &root, cursor) {
            return vec![action];
        }
        let Some(arg) = php::arg_at(&doc.text, is_blade(&doc.path), cursor) else {
            return Vec::new();
        };
        let scope = scope_of(&doc.text);
        let Some(seg) = refs::segment(&arg, cursor - arg.start, &scope) else {
            return Vec::new();
        };
        if seg.want != Want::Kind(RefKind::Translation) || arg.value.is_empty() {
            return Vec::new();
        }
        if self
            .cache
            .get(RefKind::Translation, &project, &root)
            .iter()
            .any(|e| e.key == arg.value)
        {
            return Vec::new();
        }
        self.translation_actions(&project, &root, &arg.value)
    }
}

impl Laravel {
    fn command(&mut self, id: &str, ctx: &Invocation) -> Result<Option<String>, Error> {
        match id {
            "open-hub-tab" => {
                let project = ctx.project().ok_or("open a Laravel project first")?;
                let root = project.root()?;
                self.sync_hub_files(&project, true).map_err(|why| {
                    Error::Failed(format!(
                        "the hub tab files couldn't be written to storage/openidea/hub: {why}"
                    ))
                })?;
                host::open_location(
                    &project::join(&root, &hubtabs::file_path(hubtabs::WHOLE)),
                    0,
                    0,
                )?;
                Ok(None)
            }
            "refresh-index" => {
                self.cache.clear();
                self.projects.clear();
                let project = ctx.project().ok_or("open a Laravel project first")?;
                let root = project.root()?;
                if !project::is_laravel(&project) {
                    return Err("this project is not a Laravel application".into());
                }
                show_status(&project);
                let _task = host::begin_task("Indexing the Laravel project", false);
                self.cache.load_framework(&project, &root);
                self.files.remove(&root);
                self.read_project(&project, &root, VIEW_SLICE);
                let mut counts: Vec<String> = RefKind::ALL
                    .iter()
                    .map(|k| (self.cache.get(*k, &project, &root).len(), k.label()))
                    .filter(|(n, _)| *n > 0)
                    .map(|(n, label)| format!("{n} {label}"))
                    .collect();
                let statics = self.static_candidates(&project, &root);
                let facades = statics
                    .iter()
                    .filter(|c| c.detail.starts_with("Laravel"))
                    .count();
                if facades > 0 {
                    counts.push(format!("{facades} facades and helpers"));
                }
                Ok(Some(format!("Indexed {}", counts.join(", "))))
            }
            "related-file" => self.related_file(ctx),
            "artisan-optimize-clear" => self.artisan(ctx, &["artisan", "optimize:clear"]),
            "artisan-migrate" => self.artisan(ctx, &["artisan", "migrate"]),
            "artisan-tinker" => self.artisan_terminal(ctx, "tinker"),
            "copy-query-sql" => self.copy_query_sql(ctx),
            "run-in-tinker" => self.run_selection_in_tinker(ctx),
            "extract-form-request" => self.extract_form_request(ctx),
            "artisan-serve" => self.artisan_terminal(ctx, "serve"),
            _ => Err(Error::NotFound(format!("command {id}"))),
        }
    }
}

impl openidea_plugin::Plugin for Laravel {
    fn activate(&mut self, _ctx: &Context) -> Result<(), Error> {
        self.cache.locale = host::setting_string("locale")
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "en".into());
        for kind in [
            EventKind::FileOpened,
            EventKind::FileSaved,
            EventKind::VcsChanged,
            EventKind::EditorFocused,
        ] {
            host::subscribe(kind).or_error(&format!("subscribing to {kind:?}"));
        }
        host::subscribe(EventKind::FilesChanged).or_error("subscribing to FilesChanged");
        host::subscribe_paths(EventKind::FilesChanged.name(), &WATCHED)
            .or_error("filtering FilesChanged");
        for project in host::projects()
            .or_error("listing the open projects")
            .unwrap_or_default()
        {
            if let Ok(root) = project.root()
                && let Some((project, root)) = self.laravel_project(&root)
            {
                let _task = host::begin_task("Indexing the Laravel project", false);
                self.cache.warm(&project, &root);
                self.cached_facts(&project, &root);
                self.sync_mcp(&project);
                self.sync_helpers(&project);
            }
        }
        Ok(())
    }

    fn run_command(
        &mut self,
        id: &str,
        ctx: &Invocation,
        _arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        let result = self.command(id, ctx);
        if let Err(e) = &result {
            audit::failed(&format!("command {id}"), e);
        }
        result
    }

    fn handle_events(&mut self, events: &[Event], _ctx: &Invocation) {
        let mut paths = BTreeSet::new();
        for event in events {
            match event {
                Event::FileSaved(path) => {
                    if let Some(project) = project_of(path)
                        && let Ok(root) = project.root()
                    {
                        if path.replace('\\', "/").ends_with("/config/auth.php") {
                            self.helpers_synced.remove(&root);
                            self.sync_helpers(&project);
                        }
                        self.hub_tables = None;
                        self.facts_cache = None;
                        self.live.remove(path);
                        self.reindex_usages(&project, &root, path);
                        let before = self.snapshot_keys(&project, &root, path);
                        self.cache.invalidate(&root, path);
                        let after = self.snapshot_keys(&project, &root, path);
                        self.remember_renames(path, before, after);
                        if let Ok(text) = project.read_file(path) {
                            self.remember_original(&root, path, &text, true);
                        }
                        if path.replace('\\', "/").contains("/tests/") {
                            self.pest.remove(&root);
                        }
                        if self.laravel_project(path).is_some() {
                            self.tools_on_save(&project, &root, path);
                        }
                    }
                    paths.insert(path.clone());
                }
                Event::FileOpened(path) => {
                    self.index_in_background(path);
                    if let Some(project) = project_of(path)
                        && let Ok(root) = project.root()
                        && let Ok(text) = project.read_file(path)
                    {
                        self.remember_original(&root, path, &text, true);
                    }
                    paths.insert(path.clone());
                }
                Event::FilesChanged(changed) => {
                    let all: Vec<&String> = changed
                        .created
                        .iter()
                        .chain(&changed.modified)
                        .chain(&changed.removed)
                        .collect();
                    if all.len() > MANY_CHANGES {
                        self.forget_everything();
                    } else {
                        for path in all {
                            let path = self.absolute(path);
                            if self.disk_changed(&path) {
                                paths.insert(path);
                            }
                        }
                    }
                }
                Event::VcsChanged(repo) => {
                    self.forget_everything();
                    paths.insert(repo.clone());
                }
                Event::EditorFocused(Some(path)) => {
                    self.index_in_background(path);
                    self.hub_tab_focused(path);
                    if let Some((project, _)) = self.laravel_project(path) {
                        self.sync_mcp(&project);
                        self.sync_helpers(&project);
                        let _ = self.sync_hub_files(&project, false);
                    }
                }
                _ => {}
            }
        }
        for path in paths {
            if let Some((project, root)) = self.laravel_project(&path) {
                self.cache.warm(&project, &root);
                self.cached_facts(&project, &root);
            }
            self.publish(&path);
        }
    }
}

impl openidea_plugin::syntax::SyntaxProvider for Laravel {
    fn highlights(&mut self, _language: &str, text: &str) -> Vec<openidea_plugin::syntax::Token> {
        blade_syntax::highlights(text)
    }

    fn outline(&mut self, _language: &str, text: &str) -> Vec<openidea_plugin::syntax::Symbol> {
        blade_syntax::outline(text)
    }

    fn folding(&mut self, _language: &str, text: &str) -> Vec<openidea_plugin::syntax::Fold> {
        blade_syntax::folding(text)
    }
}

#[cfg(not(target_arch = "wasm32"))]
const _: fn() -> Laravel = <Laravel as Default>::default;

#[cfg(target_arch = "wasm32")]
const _: () = {
    use openidea_plugin::syntax_bindings::exports::openidea::plugin::syntax_provider as syntax;

    struct Component;
    openidea_plugin::__plugin_exports!(Laravel, Component, openidea_plugin::bindings);
    openidea_plugin::__providers_exports!(Laravel, Component);
    openidea_plugin::__views_exports!(Laravel, Component);

    impl syntax::Guest for Component {
        fn highlights(language: String, text: String) -> Vec<syntax::Token> {
            openidea_plugin::__rt::highlights::<Laravel>(&language, &text)
        }

        fn outline(language: String, text: String) -> Vec<syntax::Symbol> {
            openidea_plugin::__rt::outline::<Laravel>(&language, &text)
        }

        fn folding(language: String, text: String) -> Vec<syntax::Fold> {
            openidea_plugin::__rt::folding::<Laravel>(&language, &text)
        }
    }

    openidea_plugin::bindings::__export_plugin_world!(Component with_types_in openidea_plugin::bindings);
    openidea_plugin::syntax_bindings::__export_syntax_world!(Component with_types_in openidea_plugin::syntax_bindings);
};

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::Plugin;
    use openidea_plugin::mock::MockHost;

    const ROUTES: &str = "<?php\nRoute::get('/', fn () => view('welcome'))->name('home');\nRoute::name('admin.')->group(function () {\n    Route::get('/users', UsersController::class)->name('users.index');\n    Route::post('/users', [UsersController::class, 'store'])->middleware('auth:web');\n});\n";
    const CONTROLLER: &str = "<?php\nclass UsersController\n{\n    public function __invoke()\n    {\n        return view('users.index', ['title' => config('app.name')]);\n    }\n\n    public function store() {}\n}\n";
    const USER: &str = "<?php\nclass User extends Authenticatable\n{\n    protected $fillable = ['name', ''];\n\n    public function posts(): HasMany\n    {\n        return $this->hasMany(Post::class);\n    }\n}\n";
    const POST: &str = "<?php\nclass Post extends Model\n{\n    public function author(): BelongsTo { return $this->belongsTo(User::class); }\n}\n";
    const MIGRATION: &str = "<?php\nreturn new class extends Migration {\n    public function up(): void {\n        Schema::create('users', function (Blueprint $table) {\n            $table->id();\n            $table->string('name');\n            $table->string('email')->unique();\n        });\n        Schema::create('posts', function (Blueprint $table) {\n            $table->id();\n            $table->string('title');\n        });\n    }\n};\n";

    fn setup() -> MockHost {
        let host = MockHost::install();
        host.add_file("artisan", "#!/usr/bin/env php\n");
        host.add_file(
            "composer.json",
            "{\"require\": {\"laravel/framework\": \"^12.0\"}}",
        );
        host.add_file("composer.lock", "{\"packages\": [{\n  \"name\": \"laravel/framework\",\n  \"version\": \"v12.3.0\"\n}]}");
        host.add_file("config/app.php", "<?php\nreturn [\n    'name' => env('APP_NAME', 'Laravel'),\n    'env' => 'local',\n];\n");
        host.add_file("config/filesystems.php", "<?php\nreturn ['disks' => ['local' => ['driver' => 'local'], 's3' => ['driver' => 's3']]];\n");
        host.add_file("config/auth.php", "<?php\nreturn ['guards' => ['web' => ['driver' => 'session'], 'api' => ['driver' => 'sanctum']]];\n");
        host.add_file("routes/web.php", ROUTES);
        host.add_file(
            "resources/views/welcome.blade.php",
            "<h1>{{ __('Welcome') }}</h1>\n@include('partials.missing')\n",
        );
        host.add_file(
            "resources/views/users/index.blade.php",
            "@extends('layouts.app')\n<a href=\"{{ route('admin.users.index') }}\">x</a>\n",
        );
        host.add_file(
            "resources/views/layouts/app.blade.php",
            "@yield('content')\n",
        );
        host.add_file(
            "resources/views/components/alert.blade.php",
            "<div></div>\n",
        );
        host.add_file(".env.example", "APP_NAME=Laravel\n");
        host.add_file("app/Http/Controllers/UsersController.php", CONTROLLER);
        host.add_file("app/Models/User.php", USER);
        host.add_file("app/Models/Post.php", POST);
        host.add_file(
            "database/migrations/2024_01_01_000000_create_tables.php",
            MIGRATION,
        );
        host.add_file("lang/en.json", "{\n    \"Hello\": \"Hello\"\n}\n");
        host
    }

    fn doc(path: &str, text: &str) -> Document {
        Document {
            path: format!("/project/{path}"),
            language: "php".into(),
            version: 1,
            text: text.into(),
        }
    }

    fn at(text: &str, needle: &str, plus: usize) -> Position {
        position(text, text.find(needle).unwrap() + plus)
    }

    #[test]
    fn inspections_report_and_fix_query_mistakes() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let text = "<?php\nclass A\n{\n    public function index()\n    {\n        $users = User::where('emial', 'x')->get();\n        foreach ($users as $user) {\n            echo $user->posts->count();\n        }\n        User::create(['name' => 'a', 'email' => 'b']);\n        return env('APP_NAME');\n    }\n}\n";
        let d = doc("app/Http/Controllers/A.php", text);
        let codes: Vec<String> = plugin
            .inspect(&d.path, text)
            .into_iter()
            .filter_map(|x| x.code)
            .collect();
        assert_eq!(
            codes,
            [
                "laravel.unknown-column",
                "laravel.n-plus-one",
                "laravel.not-fillable",
                "laravel.env-outside-config"
            ]
        );
        let titles = |needle: &str, plus: usize, plugin: &mut Laravel| -> Vec<String> {
            let p = at(text, needle, plus);
            plugin
                .code_actions(&d, Range { start: p, end: p })
                .into_iter()
                .map(|a| a.title)
                .collect()
        };
        assert_eq!(titles("emial", 1, &mut plugin), ["Change to 'email'"]);
        assert_eq!(
            titles("$user->posts", 8, &mut plugin),
            ["Eager load 'posts' with with('posts')"]
        );
        assert_eq!(
            titles("'email' =>", 2, &mut plugin),
            ["Add 'email' to $fillable of User"]
        );
        assert_eq!(
            titles("env(", 1, &mut plugin),
            ["Replace with config('app.name')"]
        );
        let p = at(text, "'email' =>", 2);
        let fix = plugin
            .code_actions(&d, Range { start: p, end: p })
            .remove(0);
        assert_eq!(fix.edits[0].0, "/project/app/Models/User.php");
        assert_eq!(fix.edits[0].1[0].text, ", 'email'");

        _host.add_file(
            "config/features.php",
            "<?php\nreturn ['debug' => (bool) env('APP_DEBUG', false)];\n",
        );
        let mut fresh = Laravel::default();
        let cast = "<?php\n$debug = env('APP_DEBUG');\n";
        let cd = doc("app/Support/Flags.php", cast);
        let p = at(cast, "env(", 1);
        let titles: Vec<String> = fresh
            .code_actions(&cd, Range { start: p, end: p })
            .into_iter()
            .map(|a| a.title)
            .collect();
        assert_eq!(titles, ["Replace with config('features.debug')"]);
    }

    fn refactor(plugin: &mut Laravel, d: &Document, needle: &str, plus: usize) -> Vec<CodeAction> {
        let p = at(&d.text, needle, plus);
        plugin
            .code_actions(d, Range { start: p, end: p })
            .into_iter()
            .filter(|a| a.kind.as_deref() == Some("refactor"))
            .collect()
    }

    fn edited_files(action: &CodeAction) -> Vec<(String, Vec<String>)> {
        action
            .edits
            .iter()
            .map(|(p, e)| {
                (
                    p.trim_start_matches("/project/").to_owned(),
                    e.iter().map(|t| t.text.clone()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn renaming_a_route_updates_its_usages_before_and_after_saving() {
        let host = setup();
        let mut plugin = Laravel::default();
        let edited = ROUTES.replace("name('users.index')", "name('users.list')");
        let d = doc("routes/web.php", &edited);
        let actions = refactor(&mut plugin, &d, "users.list", 3);
        assert_eq!(
            actions.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(),
            ["Rename route 'admin.users.index' to 'admin.users.list' in every usage"]
        );
        let resolved = plugin.resolve_code_action(&d, actions[0].clone()).unwrap();
        assert_eq!(
            edited_files(&resolved),
            [(
                "resources/views/users/index.blade.php".to_owned(),
                vec!["admin.users.list".to_owned()]
            )]
        );

        let mut open = Laravel::default();
        open.line_markers(&doc("routes/web.php", ROUTES));
        host.add_file("routes/web.php", &edited);
        let actions = refactor(&mut open, &d, "users.list", 3);
        assert_eq!(
            actions.len(),
            1,
            "the IDE reads the edited buffer: {actions:?}"
        );
        host.add_file("routes/web.php", ROUTES);

        let mut saved = Laravel::default();
        saved.handle_events(
            &[Event::FileOpened("/project/routes/web.php".into())],
            &host.invocation(),
        );
        host.add_file("routes/web.php", &edited);
        saved.handle_events(
            &[Event::FileSaved("/project/routes/web.php".into())],
            &host.invocation(),
        );
        let actions = refactor(&mut saved, &d, "users.list", 3);
        assert_eq!(actions.len(), 1, "{actions:?}");
        assert!(
            actions[0]
                .title
                .contains("'admin.users.index' to 'admin.users.list'")
        );
    }

    #[test]
    fn renaming_a_translation_key_updates_usages_and_other_locales() {
        let host = setup();
        host.add_file("lang/pt.json", "{\n    \"Hello\": \"Olá\"\n}\n");
        host.add_file(
            "resources/views/greet.blade.php",
            "<p>{{ __('Hello') }}</p>\n",
        );
        let mut plugin = Laravel::default();
        let d = Document {
            path: "/project/lang/en.json".into(),
            language: "json".into(),
            version: 1,
            text: "{\n    \"Hi there\": \"Hello\"\n}\n".into(),
        };
        let actions = refactor(&mut plugin, &d, "Hi there", 2);
        assert_eq!(
            actions[0].title,
            "Rename translation 'Hello' to 'Hi there' in every locale and usage"
        );
        let mut files = edited_files(&plugin.resolve_code_action(&d, actions[0].clone()).unwrap());
        files.sort();
        assert_eq!(
            files,
            [
                ("lang/pt.json".to_owned(), vec!["Hi there".to_owned()]),
                (
                    "resources/views/greet.blade.php".to_owned(),
                    vec!["Hi there".to_owned()]
                )
            ]
        );
    }

    #[test]
    fn extracting_a_form_request_creates_the_class_and_rewrites_the_controller() {
        let host = setup().with_capabilities(&["project.read", "editor.read", "process:php"]);
        let runs = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = runs.clone();
        host.on_process("php", move |args, stdin| {
            sink.borrow_mut()
                .push((args.to_vec(), stdin.unwrap_or_default().to_owned()));
            Ok(openidea_plugin::ProcessOutput {
                status: Some(0),
                stdout: "ok\n".into(),
                stderr: String::new(),
            })
        });
        let mut plugin = Laravel::default();
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nuse Illuminate\\Http\\Request;\n\nclass OrderController\n{\n    public function store(Request $request)\n    {\n        $data = $request->validate(['total' => 'required']);\n        return $data;\n    }\n}\n";
        let d = doc("app/Http/Controllers/OrderController.php", text);
        let action = refactor(&mut plugin, &d, "validate", 2).remove(0);
        assert_eq!(
            action.title,
            "Extract the validation into a Form Request (StoreOrderRequest)"
        );
        let resolved = plugin.resolve_code_action(&d, action).unwrap();
        let runs = runs.borrow();
        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs[0].0.last().map(String::as_str),
            Some("app/Http/Requests/StoreOrderRequest.php")
        );
        assert!(
            runs[0]
                .1
                .contains("class StoreOrderRequest extends FormRequest")
        );
        let texts: Vec<String> = resolved.edits[0].1.iter().map(|e| e.text.clone()).collect();
        assert_eq!(
            texts,
            [
                "$request->validated()",
                "StoreOrderRequest",
                "use App\\Http\\Requests\\StoreOrderRequest;"
            ]
        );
    }

    #[test]
    fn a_moved_view_is_fixed_in_one_place_or_everywhere() {
        let host = setup();
        host.add_file(
            "resources/views/shop/orders/index.blade.php",
            "<div></div>\n",
        );
        host.add_file(
            "app/Http/Controllers/OrdersController.php",
            "<?php\nclass OrdersController\n{\n    public function index() { return view('orders.index'); }\n}\n",
        );
        let mut plugin = Laravel::default();
        let text =
            "<?php\nclass A\n{\n    public function show() { return view('orders.index'); }\n}\n";
        let d = doc("app/Http/Controllers/A.php", text);
        let p = at(text, "orders.index", 2);
        let actions = plugin.code_actions(&d, Range { start: p, end: p });
        let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Change to 'shop.orders.index'",
                "Change every usage of view 'orders.index' to 'shop.orders.index'"
            ]
        );
        assert_eq!(actions[0].edits[0].1[0].text, "shop.orders.index");
        let mut files = edited_files(&plugin.resolve_code_action(&d, actions[1].clone()).unwrap());
        files.sort();
        assert_eq!(
            files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
            [
                "app/Http/Controllers/A.php",
                "app/Http/Controllers/OrdersController.php"
            ]
        );
    }

    #[test]
    fn env_files_get_each_others_missing_keys() {
        let host = setup();
        host.add_file(".env", "APP_NAME=Shop\nSTRIPE_SECRET=sk_live_x\n");
        let mut plugin = Laravel::default();
        plugin.handle_events(
            &[Event::FileSaved("/project/.env".into())],
            &host.invocation(),
        );
        let published = host.diagnostics();
        let env: Vec<&str> = published["/project/.env"]
            .iter()
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(
            env,
            ["STRIPE_SECRET is missing from .env.example, so new checkouts won't know about it"]
        );
        assert!(
            published
                .get("/project/.env.example")
                .is_none_or(|d| d.is_empty())
        );
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    fn members_only(items: &[CompletionItem]) -> Vec<&str> {
        items
            .iter()
            .filter(|i| i.kind != Some(openidea_plugin::providers::CompletionKind::Snippet))
            .map(|i| i.label.as_str())
            .collect()
    }

    #[test]
    fn completes_route_names_inside_route_calls() {
        let _host = setup();
        let text = "<?php\nreturn redirect()->route('adm');\n";
        let items =
            Laravel::default().completion(&doc("app/X.php", text), at(text, "adm'", 3), None);
        assert_eq!(labels(&items), ["admin.users.index"]);
        assert_eq!(items[0].detail.as_deref(), Some("GET /users"));
        assert_eq!(
            items[0].replace,
            Some(Range::new(at(text, "adm'", 0), at(text, "adm'", 3)))
        );
    }

    #[test]
    fn completes_and_navigates_model_members() {
        use openidea_plugin::providers::CompletionKind;
        let host = setup();
        host.add_file(
            "app/Models/Order.php",
            "<?php\nnamespace App\\Models;\n\nclass Order extends Model\n{\n    public function user()\n    {\n        return $this->belongsTo(User::class);\n    }\n\n    public function lines()\n    {\n        return $this->hasMany(Post::class);\n    }\n\n    public function scopePaid($query) {}\n}\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nclass OrderController\n{\n    public function store(Request $request)\n    {\n        $order = Order::create(['user_id' => 1]);\n        $order->\n        $order->user->na\n        foreach ($order->lines as $line) {\n            $line->\n        }\n        $order->lines->\n    }\n}\n";
        let d = doc("app/Http/Controllers/OrderController.php", text);
        let order = plugin.completion(&d, at(text, "$order->\n", 8), None);
        let labels_of =
            |items: &[CompletionItem]| items.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
        let order_labels = labels_of(&order);
        assert!(
            order_labels.contains(&"user".to_owned()) && order_labels.contains(&"lines".to_owned()),
            "{order_labels:?}"
        );
        let user = order.iter().find(|i| i.label == "user").unwrap();
        assert_eq!(user.detail.as_deref(), Some("belongsTo → User"));
        assert_eq!(user.kind, Some(CompletionKind::Class));
        let names = plugin.completion(&d, at(text, "user->na", 8), None);
        assert_eq!(labels_of(&names), ["name"]);
        assert_eq!(names[0].kind, Some(CompletionKind::Field));
        let line = plugin.completion(&d, at(text, "$line->\n", 7), None);
        assert!(
            labels_of(&line).contains(&"title".to_owned()),
            "{:?}",
            labels_of(&line)
        );
        let collection = plugin.completion(&d, at(text, "$order->lines->", 15), None);
        let first = collection.iter().find(|i| i.label == "first").unwrap();
        assert_eq!(first.insert_text, "first($0)");
        assert!(
            first
                .detail
                .as_deref()
                .is_some_and(|d| d.ends_with("?Post")),
            "{:?}",
            first.detail
        );
        assert!(!labels_of(&collection).contains(&"title".to_owned()));
        for (code, expected, absent) in [
            ("$order->lines()->", "createMany", "attach"),
            ("$order->user()->", "associate", "createMany"),
            ("$order->lines->first()->", "title", "first"),
            ("$order->lines()->get()->", "pluck", "title"),
            (
                "$order->lines()->where('a', 1)->latest()->",
                "paginate",
                "title",
            ),
            (
                "$order->lines()->where(fn ($q) => $q->x())->first()->",
                "title",
                "first",
            ),
            ("Order::", "paid", "user_id"),
            ("Order::paid()->", "where", "user"),
            ("Order::where('status', 'x')->first()->", "user", "paid"),
            ("Order::query()->paid()->get()->", "pluck", "user"),
        ] {
            let probe = text.replace("        $order->lines->\n", &format!("        {code}\n"));
            let found = plugin.completion(
                &doc("app/Http/Controllers/OrderController.php", &probe),
                at(&probe, code, code.len()),
                None,
            );
            let found = labels_of(&found);
            assert!(
                found.contains(&expected.to_owned()) && !found.contains(&absent.to_owned()),
                "{code}: {found:?}"
            );
        }

        let found = plugin.definition(&d, at(text, "user->na", 2));
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("app/Models/User.php"), "{found:?}");
        let named = text.replace(
            "user->na
",
            "user->name;
",
        );
        let found = plugin.definition(
            &doc("app/Http/Controllers/OrderController.php", &named),
            at(&named, "user->name;", 8),
        );
        assert!(
            found
                .first()
                .is_some_and(|l| l.path.contains("database/migrations")),
            "{found:?}"
        );
    }

    #[test]
    fn cast_columns_complete_the_members_of_their_enum_or_class() {
        let host = setup();
        host.add_file(
            "app/Models/Invoice.php",
            "<?php\nnamespace App\\Models;\n\nuse App\\Casts\\AsMoney;\nuse App\\Enums\\InvoiceStatus;\n\nclass Invoice extends Model\n{\n    protected function casts(): array\n    {\n        return ['status' => InvoiceStatus::class, 'total' => AsMoney::class];\n    }\n}\n",
        );
        host.add_file(
            "app/Enums/InvoiceStatus.php",
            "<?php\nnamespace App\\Enums;\n\nuse App\\Concerns\\HasLabel;\n\nenum InvoiceStatus: string\n{\n    use HasLabel;\n\n    case Paid = 'paid';\n\n    public function color(): string\n    {\n        return 'green';\n    }\n\n    public static function fallback(): self\n    {\n        return self::Paid;\n    }\n}\n",
        );
        host.add_file(
            "app/Concerns/HasLabel.php",
            "<?php\nnamespace App\\Concerns;\n\ntrait HasLabel\n{\n    public function label(): string\n    {\n        return ucfirst($this->value);\n    }\n}\n",
        );
        host.add_file(
            "app/Casts/AsMoney.php",
            "<?php\nnamespace App\\Casts;\n\nuse App\\Support\\Money;\n\nclass AsMoney implements CastsAttributes\n{\n    public function get($model, string $key, mixed $value, array $attributes): Money\n    {\n        return new Money($value);\n    }\n}\n",
        );
        host.add_file(
            "app/Support/Money.php",
            "<?php\nnamespace App\\Support;\n\nfinal class Money\n{\n    public function __construct(public readonly int $cents) {}\n\n    public function format(): string\n    {\n        return '';\n    }\n}\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nclass InvoiceController\n{\n    public function show(Invoice $invoice)\n    {\n        $invoice->status->\n        $invoice->total->\n    }\n}\n";
        let d = doc("app/Http/Controllers/InvoiceController.php", text);
        let status = plugin.completion(&d, at(text, "status->\n", 8), None);
        let status = labels(&status);
        assert!(
            ["name", "value", "color", "label"]
                .iter()
                .all(|l| status.contains(l))
                && !status.contains(&"fallback"),
            "{status:?}"
        );
        let total = plugin.completion(&d, at(text, "total->\n", 7), None);
        let total = labels(&total);
        assert!(
            total.contains(&"cents") && total.contains(&"format"),
            "{total:?}"
        );
    }

    #[test]
    fn casts_lead_to_their_enum_or_result_class() {
        let host = setup();
        host.add_file(
            "app/Models/Invoice.php",
            "<?php\nnamespace App\\Models;\n\nuse App\\Casts\\AsMoney;\nuse App\\Casts\\Plain;\nuse App\\Enums\\InvoiceStatus;\n\nclass Invoice extends Model\n{\n    protected function casts(): array\n    {\n        return ['status' => InvoiceStatus::class, 'total' => AsMoney::class, 'rate' => Plain::class, 'paid_at' => 'datetime'];\n    }\n}\n",
        );
        host.add_file(
            "app/Enums/InvoiceStatus.php",
            "<?php\nnamespace App\\Enums;\n\nenum InvoiceStatus: string\n{\n    case Paid = 'paid';\n}\n",
        );
        host.add_file(
            "app/Casts/AsMoney.php",
            "<?php\nnamespace App\\Casts;\n\nuse App\\Support\\Money;\n\nclass AsMoney implements CastsAttributes\n{\n    public function get($model, string $key, mixed $value, array $attributes): ?Money\n    {\n        return new Money($value);\n    }\n}\n",
        );
        host.add_file(
            "app/Support/Money.php",
            "<?php\nnamespace App\\Support;\n\ninterface Money\n{\n}\n",
        );
        host.add_file(
            "app/Casts/Plain.php",
            "<?php\nnamespace App\\Casts;\n\nclass Plain implements CastsAttributes\n{\n    public function get($model, string $key, mixed $value, array $attributes): float\n    {\n        return $value / 100;\n    }\n}\n",
        );
        host.add_file(
            "database/migrations/2024_02_01_000000_create_invoices_table.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void {\n        Schema::create('invoices', function (Blueprint $table) {\n            $table->id();\n            $table->string('status');\n            $table->integer('total');\n            $table->integer('rate');\n            $table->timestamp('paid_at');\n        });\n    }\n};\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nclass C\n{\n    public function show(Invoice $invoice)\n    {\n        $invoice->status;\n        $invoice->total;\n        $invoice->rate;\n        $invoice->paid_at;\n        $invoice->\n    }\n}\n";
        let d = doc("app/Http/Controllers/C.php", text);
        let go = |plugin: &mut Laravel, needle: &str| {
            plugin
                .definition(&d, at(text, needle, 11))
                .first()
                .map(|l| {
                    (
                        l.path.rsplit('/').next().unwrap_or_default().to_owned(),
                        l.range.start.line,
                    )
                })
        };
        assert_eq!(
            go(&mut plugin, "$invoice->status"),
            Some(("InvoiceStatus.php".into(), 3))
        );
        assert_eq!(
            go(&mut plugin, "$invoice->total"),
            Some(("Money.php".into(), 3))
        );
        assert_eq!(
            go(&mut plugin, "$invoice->rate;"),
            Some(("Plain.php".into(), 3))
        );
        let migration = go(&mut plugin, "$invoice->paid_at");
        assert!(migration.is_some_and(|(p, _)| p.contains("create_invoices_table")));

        let items = plugin.completion(&d, at(text, "$invoice->\n", 10), None);
        let detail = |label: &str| {
            items
                .iter()
                .find(|i| i.label == label)
                .and_then(|i| i.detail.clone())
        };
        assert_eq!(detail("status").as_deref(), Some("string → InvoiceStatus"));
        assert_eq!(detail("total").as_deref(), Some("integer → Money"));
        assert_eq!(detail("rate").as_deref(), Some("integer → float"));
        assert_eq!(
            detail("paid_at").as_deref(),
            Some("timestamp → Carbon|null")
        );
    }

    #[test]
    fn completes_pivot_attributes() {
        let host = setup();
        host.add_file(
            "app/Models/Team.php",
            "<?php\nnamespace App\\Models;\n\nclass Team extends Model\n{\n    public function members()\n    {\n        return $this->belongsToMany(User::class)->withPivot('role')->withTimestamps();\n    }\n\n    public function subscribers()\n    {\n        return $this->belongsToMany(User::class, 'subscriptions')->as('subscription')->withPivot('plan');\n    }\n}\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nforeach ($team->members as $user) {\n    echo $user->pivot->ro;\n}\n";
        let items = plugin.completion(&doc("app/X.php", text), at(text, "pivot->ro", 9), None);
        assert_eq!(labels(&items), ["role"]);
        assert_eq!(items[0].detail.as_deref(), Some("pivot of Team::members()"));
        let text = "@foreach ($team->subscribers as $u)\n{{ $u->subscription?-> }}\n@endforeach\n";
        let items = plugin.completion(
            &doc("resources/views/x.blade.php", text),
            at(text, "?-> ", 3),
            None,
        );
        assert_eq!(labels(&items), ["team_id", "user_id", "plan"]);
        let text = "<?php\n$this->pivot->x;\n$a->b;\n";
        assert!(
            plugin
                .completion(&doc("app/X.php", text), at(text, "->b", 3), None)
                .is_empty()
        );
    }

    #[test]
    fn completes_and_navigates_folio_pages() {
        let host = setup();
        host.add_file(
            "app/Providers/FolioServiceProvider.php",
            "<?php\nFolio::path(resource_path('views/pages'))->middleware([]);\n",
        );
        host.add_file(
            "resources/views/pages/users/[User].blade.php",
            "<?php\nuse function Laravel\\Folio\\name;\n\nname('users.show');\n?>\n<h1>{{ $user->name }}</h1>\n",
        );
        host.add_file("resources/views/pages/about.blade.php", "<h1>About</h1>\n");
        let text = "<?php\nreturn to_route('users.');\n";
        let mut plugin = Laravel::default();
        let items = plugin.completion(&doc("app/X.php", text), at(text, "users.'", 6), None);
        let page = items
            .iter()
            .find(|i| i.label == "users.show")
            .expect("the page");
        assert_eq!(page.detail.as_deref(), Some("GET /users/{user}"));
        let found = plugin.definition(
            &doc("app/X.php", "<?php\nreturn to_route('users.show');\n"),
            at("<?php\nreturn to_route('users.show');\n", "users.show", 2),
        );
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("pages/users/[User].blade.php"));
        assert_eq!(found[0].range.start.line, 3);
    }

    #[test]
    fn completes_config_keys_views_and_config_values() {
        let _host = setup();
        let text = "<?php\nconfig('app.');\nview('');\nStorage::disk('');\nauth('');\n";
        let mut plugin = Laravel::default();
        let config = plugin.completion(&doc("app/X.php", text), at(text, "app.'", 4), None);
        let pairs: Vec<_> = config
            .iter()
            .map(|i| (i.label.as_str(), i.detail.as_deref()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("app.name", Some("env('APP_NAME')")),
                ("app.env", Some("'local'"))
            ]
        );
        let views = plugin.completion(&doc("app/X.php", text), at(text, "view('", 6), None);
        assert_eq!(
            labels(&views),
            ["components.alert", "layouts.app", "users.index", "welcome"]
        );
        let disks = plugin.completion(&doc("app/X.php", text), at(text, "disk('", 6), None);
        assert_eq!(labels(&disks), ["local", "s3"]);
        let guards = plugin.completion(&doc("app/X.php", text), at(text, "auth('", 6), None);
        assert_eq!(labels(&guards), ["web", "api"]);
    }

    #[test]
    fn completes_eloquent_columns_relations_and_rules() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let text = "<?php\nUser::query()->where('')->with('posts.')->get();\n$request->validate(['email' => 'required|exists:users,']);\n";
        let d = doc("app/X.php", text);
        let cols = plugin.completion(&d, at(text, "where('", 7), None);
        assert_eq!(labels(&cols), ["name", "email", "id"]);
        let rels = plugin.completion(&d, at(text, "posts.'", 6), None);
        assert_eq!(labels(&rels), ["author"]);
        let rule_cols = plugin.completion(&d, at(text, "users,'", 6), None);
        assert_eq!(labels(&rule_cols), ["name", "email", "id"]);
        let rules = plugin.completion(&d, at(text, "required|", 9), None);
        assert!(labels(&rules).contains(&"exists"));
        let model = doc("app/Models/User.php", USER);
        let fillable = plugin.completion(&model, at(USER, "''", 1), None);
        assert_eq!(labels(&fillable), ["name", "email", "id"]);
    }

    #[test]
    fn untyped_closure_parameters_get_their_model() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let cases = [
            (
                "<?php\nGate::define('x', fn (User $user) => User::query()->notBlockedBy($user)\n    ->whereDoesntHave('posts', fn($query) => $query->where('|', true))\n    ->exists());\n",
                vec!["title", "id"],
            ),
            (
                "<?php\nUser::query()->where(fn($q) => $q->where('|'))->get();\n",
                vec!["name", "email", "id"],
            ),
            (
                "<?php\nUser::with(['posts' => fn ($q) => $q->orderBy('|')])->get();\n",
                vec!["title", "id"],
            ),
            (
                "<?php\nUser::whereHas('posts', function ($q) {\n    $q->whereHas('author', fn($a) => $a->where('|'));\n})->get();\n",
                vec!["name", "email", "id"],
            ),
            (
                "<?php\nfunction show(User $user) {\n    return $user->posts()->where('|')->get();\n}\n",
                vec!["title", "id"],
            ),
            (
                "<?php\nUser::whereHas('posts', fn($q) => $q->where('title', 1))->where('|')->get();\n",
                vec!["name", "email", "id"],
            ),
            ("<?php\n$foo->where('|');\n", vec![]),
        ];
        for (raw, want) in cases {
            let cursor = raw.find('|').unwrap();
            let text = raw.replacen('|', "", 1);
            let got = plugin.completion(&doc("app/X.php", &text), position(&text, cursor), None);
            assert_eq!(labels(&got), want, "{raw}");
        }
        let text = "<?php\nUser::query()->get()->each(fn($u) => $u->);\n";
        let got = plugin.completion(&doc("app/X.php", text), at(text, "$u->", 4), None);
        assert!(labels(&got).contains(&"posts"), "{:?}", labels(&got));
        let text = "<?php\nUser::whereHas('posts', fn($q) => $q->where('title', 1))->get();\n";
        let targets = plugin.definition(&doc("app/X.php", text), at(text, "'title", 2));
        assert!(
            targets
                .iter()
                .any(|l| l.path.ends_with("create_tables.php")),
            "{targets:?}"
        );
    }

    #[test]
    fn a_gate_definition_leads_to_its_usages() {
        let host = setup();
        let provider = "<?php\nclass AuthServiceProvider {\n    public function boot() {\n        Gate::define('see-profile', fn (User $user) => true);\n    }\n}\n";
        host.add_file("app/Providers/AuthServiceProvider.php", provider);
        host.add_file(
            "app/Http/Controllers/ProfileController.php",
            "<?php\nclass ProfileController {\n    public function show() {\n        $this->authorize('see-profile');\n        return Gate::allows('see-profile') ? 1 : Gate::denies('other');\n    }\n}\n",
        );
        host.add_file(
            "resources/views/profile.blade.php",
            "@can('see-profile')\nx\n@endcan\n",
        );
        host.add_file(
            "routes/api.php",
            "<?php\nRoute::get('/p', fn () => 1)->middleware('can:see-profile');\n",
        );
        let mut plugin = Laravel::default();
        let d = doc("app/Providers/AuthServiceProvider.php", provider);
        let mut found: Vec<(String, u32)> = plugin
            .definition(&d, at(provider, "see-profile", 3))
            .into_iter()
            .map(|l| {
                (
                    l.path.trim_start_matches("/project/").to_owned(),
                    l.range.start.line,
                )
            })
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                ("app/Http/Controllers/ProfileController.php".to_owned(), 3),
                ("app/Http/Controllers/ProfileController.php".to_owned(), 4),
                ("resources/views/profile.blade.php".to_owned(), 0),
                ("routes/api.php".to_owned(), 1),
            ]
        );
    }

    #[test]
    fn models_extending_a_model_inherit_its_relations_and_scopes() {
        let host = setup();
        host.add_file(
            "app/Models/Product.php",
            "<?php\nnamespace App\\Models;\n\nclass Product extends Model\n{\n    public function category()\n    {\n        return $this->belongsTo(Category::class);\n    }\n\n    public function scopePublished($query) {}\n}\n",
        );
        host.add_file(
            "app/Models/DigitalProduct.php",
            "<?php\nnamespace App\\Models;\n\nclass DigitalProduct extends Product\n{\n    public function downloads()\n    {\n        return $this->hasMany(Download::class);\n    }\n}\n",
        );
        host.add_file(
            "app/Models/Ebook.php",
            "<?php\nnamespace App\\Models;\n\nclass Ebook extends DigitalProduct\n{\n}\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nclass ShopController\n{\n    public function show()\n    {\n        $book = Ebook::find(1);\n        $book->\n        Ebook::published();\n    }\n}\n";
        let d = doc("app/Http/Controllers/ShopController.php", text);
        let found = plugin.completion(&d, at(text, "$book->\n", 7), None);
        let found = labels(&found);
        assert!(
            found.contains(&"category") && found.contains(&"downloads"),
            "{found:?}"
        );
        let scope = plugin.definition(&d, at(text, "published()", 2));
        assert!(
            scope
                .iter()
                .any(|l| l.path.ends_with("app/Models/Product.php")),
            "{scope:?}"
        );
    }

    #[test]
    fn a_scope_definition_leads_to_its_usages() {
        let host = setup();
        let model = "<?php\nnamespace App\\Models;\n\nclass Product extends Model\n{\n    public function scopePublished(Builder $query): void\n    {\n        $query->whereNotNull('published_at');\n    }\n\n    #[Scope]\n    protected function cheap(Builder $query): void\n    {\n        $query->where('price', '<', 10);\n    }\n\n    public function latestPublished()\n    {\n        return static::published()->latest();\n    }\n}\n";
        host.add_file("app/Models/Product.php", model);
        host.add_file(
            "app/Http/Controllers/ProductController.php",
            "<?php\nclass ProductController {\n    public function index() {\n        $a = Product::published()->get();\n        $b = Product::query()->cheap()->published()->get();\n        $c = Order::published();\n        $d = $published;\n    }\n}\n",
        );
        let mut plugin = Laravel::default();
        let d = doc("app/Models/Product.php", model);
        let lines_of = |found: Vec<Location>| {
            let mut out: Vec<(String, u32)> = found
                .into_iter()
                .map(|l| {
                    (
                        l.path.trim_start_matches("/project/").to_owned(),
                        l.range.start.line,
                    )
                })
                .collect();
            out.sort();
            out
        };
        assert_eq!(
            lines_of(plugin.definition(&d, at(model, "scopePublished", 6))),
            [
                ("app/Http/Controllers/ProductController.php".to_owned(), 3),
                ("app/Http/Controllers/ProductController.php".to_owned(), 4),
                ("app/Models/Product.php".to_owned(), 18),
            ]
        );
        assert_eq!(
            lines_of(plugin.definition(&d, at(model, "cheap(Builder", 2))),
            [("app/Http/Controllers/ProductController.php".to_owned(), 4)]
        );
        let controller = "<?php\nclass ProductController {\n    public function index(Product $product) {\n        $a = Product::published()->get();\n        $b = $product->cheap();\n        $c = Product::query()->published();\n    }\n}\n";
        host.add_file("app/Http/Controllers/ProductController.php", controller);
        let c = doc("app/Http/Controllers/ProductController.php", controller);
        assert_eq!(
            lines_of(plugin.definition(&c, at(controller, "published()->get", 3))),
            [("app/Models/Product.php".to_owned(), 5)]
        );
        assert_eq!(
            lines_of(plugin.definition(&c, at(controller, "cheap()", 2))),
            [("app/Models/Product.php".to_owned(), 11)]
        );
        assert_eq!(
            lines_of(plugin.definition(&c, at(controller, "published();", 2))),
            [("app/Models/Product.php".to_owned(), 5)]
        );
    }

    #[test]
    fn completes_middleware_guards_and_controller_methods() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let text = "<?php\nRoute::middleware('auth:')->get('/x', [UsersController::class, '']);\n";
        let d = doc("routes/api.php", text);
        let guards = plugin.completion(&d, at(text, "auth:'", 5), None);
        assert_eq!(labels(&guards), ["web", "api"]);
        let methods = plugin.completion(&d, at(text, ", '']", 3), None);
        assert_eq!(labels(&methods), ["__invoke", "store"]);
        let mw = plugin.completion(&d, at(text, "auth:", 2), None);
        assert!(labels(&mw).contains(&"auth"));
    }

    #[test]
    fn completes_and_navigates_blade_components() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let text = "<div>\n    <x-al\n</div>\n";
        let d = doc("resources/views/home.blade.php", text);
        let items = plugin.completion(&d, at(text, "<x-al", 5), None);
        assert_eq!(labels(&items), ["alert"]);
        let defs = plugin.definition(&d, at(text, "<x-al", 4));
        assert!(defs.is_empty());
        let text = "<x-alert type=\"error\" />\n";
        let d = doc("resources/views/home.blade.php", text);
        let defs = plugin.definition(&d, at(text, "alert", 2));
        assert_eq!(
            defs[0].path,
            "/project/resources/views/components/alert.blade.php"
        );
    }

    #[test]
    fn goes_to_definitions() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let controller = doc("app/Http/Controllers/UsersController.php", CONTROLLER);
        let view = plugin.definition(&controller, at(CONTROLLER, "users.index", 2));
        assert_eq!(
            view[0].path,
            "/project/resources/views/users/index.blade.php"
        );
        let config = plugin.definition(&controller, at(CONTROLLER, "app.name", 1));
        assert_eq!(
            (config[0].path.as_str(), config[0].range.start),
            ("/project/config/app.php", Position::new(2, 5))
        );
        let blade_text =
            "@extends('layouts.app')\n<a href=\"{{ route('admin.users.index') }}\">x</a>\n";
        let blade = doc("resources/views/users/index.blade.php", blade_text);
        let route = plugin.definition(&blade, at(blade_text, "admin.users", 3));
        assert_eq!(
            (route[0].path.as_str(), route[0].range.start.line),
            ("/project/routes/web.php", 3)
        );
        let routes = doc("routes/web.php", ROUTES);
        let method = plugin.definition(&routes, at(ROUTES, "'store'", 2));
        assert_eq!(
            method[0].path,
            "/project/app/Http/Controllers/UsersController.php"
        );
        let by_name = plugin.definition(&routes, at(ROUTES, "users.index", 3));
        assert_eq!(
            (by_name[0].path.as_str(), by_name[0].range.start.line),
            ("/project/app/Http/Controllers/UsersController.php", 3)
        );
        let by_uri = plugin.definition(&routes, at(ROUTES, "/users', [", 2));
        assert_eq!(
            (by_uri[0].path.as_str(), by_uri[0].range.start.line),
            ("/project/app/Http/Controllers/UsersController.php", 8)
        );
        let relation_text = "<?php\nPost::with('author')->get();\n";
        let rel_def = plugin.definition(
            &doc("app/X.php", relation_text),
            at(relation_text, "author", 1),
        );
        assert_eq!(rel_def[0].path, "/project/app/Models/Post.php");
    }

    #[test]
    fn marks_views_and_controller_routes_in_the_gutter() {
        let _host = setup();
        let markers = Laravel::default()
            .line_markers(&doc("app/Http/Controllers/UsersController.php", CONTROLLER));
        let summary: Vec<_> = markers
            .iter()
            .map(|m| (m.line, m.tooltip.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                (5, "Go to view resources/views/users/index.blade.php"),
                (3, "Route: GET /users · admin.users.index"),
                (8, "Route: POST /users"),
                (1, "UsersController is used in 1 file\nroutes/web.php:4"),
            ]
        );
    }

    #[test]
    fn offers_to_add_missing_translations() {
        let _host = setup();
        let text = "<?php\n__('Good morning');\n";
        let actions = Laravel::default().code_actions(
            &doc("app/X.php", text),
            Range::new(at(text, "Good", 1), at(text, "Good", 1)),
        );
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Add 'Good morning' to lang/en.json");
        assert_eq!(
            actions[0].edits[0].1[0].text,
            ",\n    \"Good morning\": \"Good morning\""
        );
    }

    #[test]
    fn adds_missing_translations_to_resources_lang_on_older_laravel() {
        let host = setup();
        host.add_file(
            "resources/lang/en/messages.php",
            "<?php\n\nreturn [\n    'welcome' => 'Welcome',\n];\n",
        );
        let text = "<?php\n__('messages.brand_new');\n";
        let actions = Laravel::default().code_actions(
            &doc("app/X.php", text),
            Range::new(at(text, "brand", 1), at(text, "brand", 1)),
        );
        let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
        assert!(
            titles.contains(&"Add 'messages.brand_new' to resources/lang/en/messages.php"),
            "{titles:?}"
        );
    }

    #[test]
    fn reports_unknown_views_when_files_open() {
        let host = setup();
        let mut plugin = Laravel::default();
        plugin
            .activate(&host.context("pedro.laravel-openidea"))
            .unwrap();
        let path = "/project/resources/views/welcome.blade.php";
        plugin.handle_events(&[Event::FileOpened(path.into())], &host.invocation());
        let diagnostics = host.diagnostics();
        let items = &diagnostics[path];
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].message, "View 'partials.missing' not found");
        assert_eq!(items[0].range.start, Position::new(1, 10));
        assert_eq!(host.status_items()[STATUS_ID].text, "Laravel 12.3.0");
    }

    #[test]
    fn supports_openapi_attributes() {
        let host = setup();
        host.add_file(
            "app/Docs/UserSchema.php",
            "<?php\nuse OpenApi\\Attributes as OA;\n#[OA\\Schema(schema: 'UserSchema', required: ['id'], properties: [new OA\\Property(property: 'id', type: 'integer')])]\nclass UserSchema {}\n#[OA\\SecurityScheme(securityScheme: 'bearerAuth', type: 'http')]\nclass Security {}\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nuse OpenApi\\Attributes as OA;\n#[OA\\Get(path: '/users/{user}', security: [['' => []]], parameters: [new OA\\Parameter(name: '', in: 'path')], responses: [new OA\\Response(response: 200, content: new OA\\JsonContent(ref: '#/components/schemas/User'))])]\n#[OA\\Schema(schema: 'Local', required: [''], properties: [new OA\\Property(property: 'email')])]\nclass C {}\n";
        let d = doc("app/Http/Controllers/C.php", text);
        let refs = plugin.completion(&d, at(text, "/User'", 5), None);
        assert_eq!(labels(&refs), ["#/components/schemas/UserSchema"]);
        let security = plugin.completion(&d, at(text, "[['", 3), None);
        assert_eq!(labels(&security), ["bearerAuth"]);
        let params = plugin.completion(&d, at(text, "name: '", 7), None);
        assert_eq!(labels(&params), ["user"]);
        let required = plugin.completion(&d, at(text, "required: ['", 12), None);
        assert_eq!(labels(&required), ["email"]);
        let paths = plugin.completion(&d, at(text, "path: '", 7), None);
        assert!(labels(&paths).contains(&"/users"));
        let ok = "<?php\nuse OpenApi\\Attributes as OA;\nnew OA\\JsonContent(ref: '#/components/schemas/UserSchema');\nnew OA\\JsonContent(ref: '#/components/schemas/Missing');\n";
        let defs = plugin.definition(&doc("app/X.php", ok), at(ok, "UserSchema'", 2));
        assert_eq!(defs[0].path, "/project/app/Docs/UserSchema.php");
        let problems = plugin.inspect("/project/app/X.php", ok);
        assert_eq!(problems.len(), 1);
        assert_eq!(
            problems[0].message,
            "OpenAPI component '#/components/schemas/Missing' not found"
        );
    }

    #[test]
    fn completes_model_attributes_and_members() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let attr = "<?php\n#[Fillable([''])]\n#[Table('users')]\nclass User extends Authenticatable\n{\n    protected $fil\n}\n";
        let d = doc("app/Models/User.php", attr);
        let cols = plugin.completion(&d, at(attr, "(['", 3), None);
        assert_eq!(labels(&cols), ["name", "email", "id"]);
        assert_eq!(
            cols[0].kind,
            Some(openidea_plugin::providers::CompletionKind::Field)
        );
        let members = plugin.completion(&d, at(attr, "$fil\n", 4), None);
        assert_eq!(labels(&members), ["$fillable"]);
        assert_eq!(
            members[0].insert_text,
            "protected $fillable = [\n        'name',\n        'email',\n    ];"
        );
        assert_eq!(
            members[0].replace.unwrap().start,
            at(attr, "protected $fil", 0)
        );
        let post = "<?php\nclass Post extends Model\n{\n    use HasFactory;\n    aut\n}\n";
        let rel = plugin.completion(
            &doc("app/Models/Post.php", post),
            at(post, "aut\n", 3),
            None,
        );
        assert!(members_only(&rel).is_empty(), "author() already exists");
        let user = "<?php\nclass User extends Authenticatable\n{\n    pos\n}\n";
        let has_many = plugin.completion(
            &doc("app/Models/User.php", user),
            at(user, "pos\n", 3),
            None,
        );
        assert!(
            labels(&has_many).is_empty(),
            "posts() already exists in the indexed User model"
        );
    }

    #[test]
    fn relation_items_are_not_callable() {
        let _host = setup();
        let text = "<?php\nUser::with('')->get();\n";
        let items = Laravel::default().completion(&doc("app/X.php", text), at(text, "('", 2), None);
        assert_eq!(labels(&items), ["posts"]);
        assert_eq!(
            items[0].kind,
            Some(openidea_plugin::providers::CompletionKind::Reference)
        );
    }

    #[test]
    fn completes_blade_directives() {
        let host = setup();
        host.add_file(
            "app/Providers/BladeServiceProvider.php",
            "<?php
Blade::if('admin', fn () => true);
",
        );
        let text = "<div>
    @fore
    @adm
</div>
";
        let d = doc("resources/views/home.blade.php", text);
        let mut plugin = Laravel::default();
        let items = plugin.completion(&d, at(text, "@fore", 5), None);
        assert_eq!(
            labels(&items),
            [
                "@foreach",
                "@endforeach",
                "@forelse",
                "@endforelse",
                "@foreach … @end",
                "@forelse … @end"
            ]
        );
        assert_eq!(
            items[4].insert_text,
            "@foreach (${1:\\$items} as ${2:\\$item})\n\t$0\n@endforeach"
        );
        assert_eq!(items[0].replace.unwrap().start, at(text, "@fore", 0));
        let custom = plugin.completion(&d, at(text, "@adm", 4), None);
        assert_eq!(
            labels(&custom),
            ["@admin", "@elseadmin", "@unlessadmin", "@endadmin"]
        );
    }

    #[test]
    fn completes_the_psr4_namespace_of_the_file() {
        let host = setup();
        host.add_file(
            "composer.json",
            r#"{"require": {"laravel/framework": "^12.0"}, "autoload": {"psr-4": {"App\\": "app/"}}, "autoload-dev": {"psr-4": {"Tests\\": "tests/"}}}"#,
        );
        let mut plugin = Laravel::default();
        let text = "<?php\n\nnamespace \n";
        let items = plugin.completion(
            &doc("app/Http/Controllers/Admin/UserController.php", text),
            at(text, "namespace ", 10),
            None,
        );
        assert_eq!(labels(&items), ["App\\Http\\Controllers\\Admin"]);
        assert_eq!(items[0].insert_text, "App\\Http\\Controllers\\Admin;");
        let test = "<?php\nnamespace Tests\\F";
        let items = plugin.completion(
            &doc("tests/Feature/OrderTest.php", test),
            position(test, test.len()),
            None,
        );
        assert_eq!(labels(&items), ["Tests\\Feature"]);
    }

    #[test]
    fn checks_and_completes_enum_values_in_where_clauses() {
        let host = setup();
        host.add_file(
            "app/Models/Order.php",
            "<?php\nnamespace App\\Models;\n\nuse App\\Enums\\OrderStatus;\n\nclass Order extends Model\n{\n    protected function casts(): array\n    {\n        return ['status' => OrderStatus::class];\n    }\n}\n",
        );
        host.add_file(
            "app/Enums/OrderStatus.php",
            "<?php\nnamespace App\\Enums;\n\nenum OrderStatus: string\n{\n    case Pending = 'pending';\n    case Paid = 'paid';\n    case Refunded = 'refunded';\n}\n",
        );
        host.add_file(
            "database/migrations/2024_02_01_000000_create_orders.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void {\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->string('status');\n        });\n    }\n};\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nOrder::where('status', 'refused')->get();\nOrder::query()->where('status', '!=', 'paid')->orWhereIn('status', ['pending', 'lost'])->get();\nOrder::where('status', '');\n";
        let problems = plugin.inspect("/project/app/X.php", text);
        let messages: Vec<&str> = problems
            .iter()
            .filter(|p| p.code.as_deref() == Some("laravel.unknown-enum-value"))
            .map(|p| p.message.as_str())
            .collect();
        assert_eq!(
            messages,
            [
                "Column 'status' can never be 'refused': it is cast to OrderStatus ('pending', 'paid', 'refunded')",
                "Column 'status' can never be 'lost': it is cast to OrderStatus ('pending', 'paid', 'refunded')",
            ]
        );
        let d = doc("app/X.php", text);
        let actions = plugin.code_actions(
            &d,
            Range::new(at(text, "refused", 2), at(text, "refused", 2)),
        );
        assert!(actions.iter().any(|a| a.title == "Change to 'refunded'"));
        let values = plugin.completion(&d, at(text, "'status', '')", 11), None);
        assert_eq!(labels(&values), ["pending", "paid", "refunded"]);
        assert_eq!(values[0].detail.as_deref(), Some("OrderStatus::Pending"));
        let op = "<?php\nOrder::where('status', '<>', 'p');\n";
        let values = plugin.completion(&doc("app/Y.php", op), at(op, "'p')", 2), None);
        assert_eq!(labels(&values), ["pending", "paid"]);
        let columns = plugin.completion(&doc("app/Y.php", op), at(op, "'status'", 3), None);
        assert!(labels(&columns).contains(&"status"));
    }

    #[test]
    fn enum_warnings_follow_the_text_as_it_is_typed() {
        let host = setup();
        host.add_file(
            "app/Models/Order.php",
            "<?php\nnamespace App\\Models;\n\nuse App\\Enums\\OrderStatus;\n\nclass Order extends Model\n{\n    protected $casts = ['status' => OrderStatus::class];\n}\n",
        );
        host.add_file(
            "app/Enums/OrderStatus.php",
            "<?php\nnamespace App\\Enums;\n\nenum OrderStatus: string\n{\n    case Paid = 'paid';\n}\n",
        );
        host.add_file(
            "database/migrations/2024_02_01_000000_create_orders.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void {\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->string('status');\n        });\n    }\n};\n",
        );
        let mut plugin = Laravel::default();
        let path = "/project/app/X.php";
        host.add_file("app/X.php", "<?php\nOrder::where('status', 'paid');\n");
        plugin.handle_events(&[Event::FileOpened(path.into())], &host.invocation());
        let enum_problems = |host: &MockHost| {
            host.diagnostics()
                .get(path)
                .map(|d| {
                    d.iter()
                        .filter(|d| d.code.as_deref() == Some("laravel.unknown-enum-value"))
                        .count()
                })
                .unwrap_or(0)
        };
        assert_eq!(enum_problems(&host), 0);
        let mut typed = doc("app/X.php", "<?php\nOrder::where('status', 'pai');\n");
        typed.version = 2;
        plugin.line_markers(&typed);
        assert_eq!(enum_problems(&host), 1, "shows up while typing");
        typed.text = "<?php\nOrder::where('status', 'paid');\n".into();
        typed.version = 3;
        plugin.code_actions(&typed, Range::default());
        assert_eq!(enum_problems(&host), 0, "goes away while typing");
    }

    #[test]
    fn factories_complete_their_states_and_open_on_ctrl_click() {
        let host = setup();
        host.add_file(
            "database/factories/UserFactory.php",
            "<?php\nnamespace Database\\Factories;\n\nclass UserFactory extends Factory\n{\n    public function definition(): array { return []; }\n    public function unverified(): static { return $this; }\n}\n",
        );
        let mut plugin = Laravel::default();
        let text =
            "<?php\nUser::factory()->;\nUser::factory()->unverified()->create()->;\nUser::fac;\n";
        let d = doc("app/X.php", text);
        let states = plugin.completion(&d, at(text, "factory()->;", 11), None);
        let names = labels(&states);
        assert!(
            names.contains(&"unverified") && names.contains(&"count") && names.contains(&"create"),
            "{names:?}"
        );
        assert_eq!(names[0], "unverified");
        let made = plugin.completion(&d, at(text, "create()->;", 10), None);
        assert!(labels(&made).contains(&"email"), "{:?}", labels(&made));
        let statics = plugin.completion(&d, at(text, "User::fac;", 9), None);
        assert!(labels(&statics).contains(&"factory"));
        let found = plugin.definition(&d, at(text, "factory()->;", 2));
        assert_eq!(found[0].path, "/project/database/factories/UserFactory.php");
        assert_eq!(found[0].range.start.line, 3);
        let model = "<?php\nnamespace App\\Models;\n\nclass User extends Authenticatable\n{\n    use HasFactory;\n}\n";
        let found = plugin.definition(
            &doc("app/Models/User.php", model),
            at(model, "HasFactory;", 3),
        );
        assert_eq!(found[0].path, "/project/database/factories/UserFactory.php");
        assert_eq!(found[0].range.start.line, 3);
    }

    #[test]
    fn callbacks_complete_typed_closures_with_imports() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let first = |plugin: &mut Laravel, text: &str| {
            let items =
                plugin.completion(&doc("app/X.php", text), position(text, text.len()), None);
            items
                .into_iter()
                .next()
                .map(|i| (i.label, i.insert_text, i.additional_edits))
        };
        let (label, insert, edits) = first(
            &mut plugin,
            "<?php\nnamespace App\\Http;\n\nUser::factory()->afterCreating(fn",
        )
        .unwrap();
        assert_eq!(label, "fn (User $user) =>");
        assert_eq!(insert, "fn (User \\$user) => $0");
        assert_eq!(edits[0].text, "\n\nuse App\\Models\\User;");
        let (label, _, edits) =
            first(&mut plugin, "<?php\nnamespace App\\Http;\n\nUser::where(fn").unwrap();
        assert_eq!(label, "fn (Builder $query) =>");
        assert!(
            edits[0]
                .text
                .contains("use Illuminate\\Database\\Eloquent\\Builder;")
        );
        let (label, _, _) =
            first(&mut plugin, "<?php\n$users = User::all();\n$users->map(f").unwrap();
        assert_eq!(label, "fn (User $user) =>");
        let (label, _, _) = first(&mut plugin, "<?php\nSchema::create('posts', fn").unwrap();
        assert_eq!(label, "fn (Blueprint $table) =>");
        let (label, insert, _) = first(&mut plugin, "<?php\nDB::transaction(function").unwrap();
        assert_eq!(label, "fn () =>");
        assert!(insert.starts_with("fn () =>"));
    }

    #[test]
    fn relation_snippets_use_the_models_the_migrations_point_to() {
        let host = setup();
        host.add_file(
            "app/Models/Order.php",
            "<?php\nnamespace App\\Models;\n\nclass Order extends Model\n{\n}\n",
        );
        host.add_file(
            "database/migrations/2024_02_01_000000_create_orders.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void {\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->foreignId('user_id');\n        });\n    }\n};\n",
        );
        let mut plugin = Laravel::default();
        let user =
            "<?php\nnamespace App\\Models;\n\nclass User extends Authenticatable\n{\n    hasm\n}\n";
        let items = plugin.completion(
            &doc("app/Models/User.php", user),
            at(user, "hasm\n", 4),
            None,
        );
        let item = items
            .iter()
            .find(|i| i.label == "hasmany orders → Order")
            .unwrap_or_else(|| panic!("{:?}", labels(&items)));
        assert!(
            item.insert_text
                .contains("public function orders(): HasMany")
        );
        assert!(item.insert_text.contains("hasMany(Order::class)"));
        assert!(!labels(&items).contains(&"hasmany"), "no placeholder model");
        let order =
            "<?php\nnamespace App\\Models;\n\nclass Order extends Model\n{\n    belongsto\n}\n";
        let items = plugin.completion(
            &doc("app/Models/Order.php", order),
            at(order, "belongsto\n", 9),
            None,
        );
        assert!(
            labels(&items).contains(&"belongsto user → User"),
            "{:?}",
            labels(&items)
        );
    }

    #[test]
    fn the_gutter_leads_to_the_usages_of_classes_and_scopes() {
        use openidea_plugin::views::{ViewEvent, Views};
        let host = setup();
        host.add_file(
            "app/Http/Controllers/ProfileController.php",
            "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\User;\n\nclass ProfileController\n{\n    public function show(User $user) { return UserResource::make(User::active()->first()); }\n}\n",
        );
        host.add_file(
            "app/Jobs/Prune.php",
            "<?php\nnamespace App\\Jobs;\n\nuse App\\Models\\User;\n\nclass Prune\n{\n    // User::active()->delete();\n    public function handle() {}\n}\n",
        );
        host.add_file(
            "app/Http/Resources/UserResource.php",
            "<?php\nnamespace App\\Http\\Resources;\n\nclass UserResource extends JsonResource\n{\n}\n",
        );
        host.add_file(
            "tests/Feature/UserTest.php",
            "<?php\nit('works', fn () => User::factory()->create());\n",
        );
        let model_text = "<?php\nclass User extends Authenticatable\n{\n    protected $fillable = ['name', ''];\n\n    public function scopeActive($query) { return $query; }\n\n    public function posts(): HasMany\n    {\n        return $this->hasMany(Post::class);\n    }\n}\n";
        host.add_file("app/Models/User.php", model_text);
        let mut plugin = Laravel::default();
        let model = doc("app/Models/User.php", model_text);
        plugin.handle_events(
            &[Event::FileOpened("/project/app/Models/User.php".into())],
            &host.invocation(),
        );
        let markers = plugin.line_markers(&model);
        let class = markers.iter().find(|m| m.line == 1).unwrap();
        assert!(
            class.tooltip.starts_with("User is used in 3 files"),
            "{}",
            class.tooltip
        );
        assert!(
            class
                .tooltip
                .contains("app/Http/Controllers/ProfileController.php:8")
        );
        assert!(!class.tooltip.contains("Prune"), "{}", class.tooltip);
        assert_eq!(
            class.target.as_ref().unwrap().path,
            "/project/app/Http/Controllers/ProfileController.php"
        );
        assert_eq!(class.target.as_ref().unwrap().range.start.line, 7);
        let scope = markers.iter().find(|m| m.line == 5).unwrap();
        assert_eq!(
            scope.tooltip.lines().next(),
            Some("scope active() is used in 1 file")
        );
        let resource_text = "<?php\nnamespace App\\Http\\Resources;\n\nclass UserResource extends JsonResource\n{\n}\n";
        let resource = doc("app/Http/Resources/UserResource.php", resource_text);
        let markers = plugin.line_markers(&resource);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].line, 3);
        let found = plugin.definition(&model, at(model_text, "User extends", 1));
        assert!(
            found.iter().all(|l| !l.path.ends_with("Prune.php")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|l| l.path.ends_with("ProfileController.php") && l.range.start.line == 7)
        );
        assert!(
            !found
                .iter()
                .any(|l| l.path.ends_with("ProfileController.php") && l.range.start.line == 3),
            "imports are not usages"
        );
        plugin
            .handle(
                "hub",
                ViewEvent::Message(r#"{"type":"model","name":"User","instance":"t"}"#.into()),
                &host.invocation(),
            )
            .unwrap();
        let (_, message) = host.view_messages().pop().unwrap();
        let message: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(message["type"], "modelresources");
        let titles: Vec<&str> = message["groups"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|g| g["title"].as_str())
            .collect();
        assert_eq!(
            titles,
            [
                "Model",
                "Migrations",
                "Relations",
                "Scopes",
                "Controllers",
                "Models",
                "Tests"
            ]
        );
    }

    #[test]
    fn enums_and_static_classes_complete_with_their_double_colon() {
        let host = setup();
        host.add_file(
            "app/Enums/OrderStatus.php",
            "<?php\nnamespace App\\Enums;\n\nenum OrderStatus: string\n{\n    case Paid = 'paid';\n}\n",
        );
        host.add_file(
            "app/Support/Money.php",
            "<?php\nnamespace App\\Support;\n\nclass Money\n{\n    public static function format(int $cents): string { return ''; }\n}\n",
        );
        host.add_file(
            "app/Services/Billing.php",
            "<?php\nnamespace App\\Services;\n\nclass Billing\n{\n    public function charge() {}\n}\n",
        );
        let mut plugin = Laravel::default();
        plugin.handle_events(
            &[Event::FileOpened(
                "/project/app/Enums/OrderStatus.php".into(),
            )],
            &host.invocation(),
        );
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nclass OrdersController\n{\n    public function index()\n    {\n        $status = Ord\n        $total = Mon\n        $b = Bil\n    }\n}\n";
        let d = doc("app/Http/Controllers/OrdersController.php", text);
        let items = plugin.completion(&d, at(text, "Ord\n", 3), None);
        let item = items.iter().find(|i| i.label == "OrderStatus::").unwrap();
        assert_eq!(item.insert_text, "OrderStatus::");
        assert_eq!(
            item.additional_edits[0].text,
            "\n\nuse App\\Enums\\OrderStatus;"
        );
        let items = plugin.completion(&d, at(text, "Mon\n", 3), None);
        assert!(labels(&items).contains(&"Money::"), "{:?}", labels(&items));
        let items = plugin.completion(&d, at(text, "Bil\n", 3), None);
        assert!(
            !labels(&items).contains(&"Billing::"),
            "{:?}",
            labels(&items)
        );
        let imported = "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Enums\\OrderStatus;\n\nclass OrdersController\n{\n    public function index()\n    {\n        Ord\n    }\n}\n";
        let d = doc("app/Http/Controllers/OrdersController.php", imported);
        let items = plugin.completion(&d, at(imported, "Ord\n", 3), None);
        let item = items.iter().find(|i| i.label == "OrderStatus::");
        assert!(item.is_some(), "{:?}", labels(&items));
        assert!(item.unwrap().additional_edits.is_empty());
        host.add_file(
            "config/app.php",
            "<?php\nreturn [\n    'name' => 'x',\n    'aliases' => [\n        'Cache' => Illuminate\\Support\\Facades\\Cache::class,\n    ],\n];\n",
        );
        let facade = "<?php\nnamespace App\\Http\\Controllers;\n\nuse Illuminate\\Support\\Facades\\Cache;\n\nclass OrdersController\n{\n    public function index()\n    {\n        Cac\n    }\n}\n";
        let d = doc("app/Http/Controllers/OrdersController.php", facade);
        let items = plugin.completion(&d, at(facade, "Cac\n", 3), None);
        assert!(labels(&items).contains(&"Cache::"), "{:?}", labels(&items));
        let model = "<?php\nnamespace App\\Models;\n\nclass Order extends Model\n{\n    protected $casts = [\n        'status' => Ord\n    ];\n}\n";
        let items = plugin.completion(
            &doc("app/Models/Order.php", model),
            at(model, "Ord\n", 3),
            None,
        );
        assert_eq!(labels(&items), ["OrderStatus::class"]);
        assert_eq!(items[0].insert_text, "OrderStatus::class");
    }

    #[test]
    fn ctrl_click_on_a_cast_column_opens_its_enum_or_value_class() {
        let host = setup();
        host.add_file(
            "app/Enums/OrderStatus.php",
            "<?php\nnamespace App\\Enums;\n\nenum OrderStatus: string\n{\n    case Paid = 'paid';\n}\n",
        );
        host.add_file(
            "app/Support/Money.php",
            "<?php\nnamespace App\\Support;\n\nclass Money\n{\n    public function __construct(public int $cents) {}\n}\n",
        );
        host.add_file(
            "app/Casts/MoneyCast.php",
            "<?php\nnamespace App\\Casts;\n\nuse App\\Support\\Money;\n\nclass MoneyCast implements CastsAttributes\n{\n    public function get($model, $key, $value, $attributes): Money\n    {\n        return new Money((int) $value);\n    }\n}\n",
        );
        host.add_file(
            "app/Models/Order.php",
            "<?php\nnamespace App\\Models;\n\nuse App\\Casts\\MoneyCast;\nuse App\\Enums\\OrderStatus;\nuse Illuminate\\Database\\Eloquent\\Model;\n\nclass Order extends Model\n{\n    protected $casts = [\n        'status' => OrderStatus::class,\n        'total' => MoneyCast::class,\n    ];\n}\n",
        );
        host.add_file(
            "database/migrations/2024_01_02_000000_create_orders_table.php",
            "<?php\nSchema::create('orders', function (Blueprint $table) {\n    $table->id();\n    $table->string('status');\n    $table->integer('total');\n});\n",
        );
        let mut plugin = Laravel::default();
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Order;\n\nclass A\n{\n    public function b(Order $order)\n    {\n        $order->status;\n        $order->total;\n    }\n}\n";
        let d = doc("app/Http/Controllers/A.php", text);
        let found = plugin.definition(&d, at(text, ">status", 2));
        assert_eq!(
            found[0].path, "/project/app/Enums/OrderStatus.php",
            "{found:?}"
        );
        let found = plugin.definition(&d, at(text, ">total", 2));
        assert_eq!(found[0].path, "/project/app/Support/Money.php", "{found:?}");
    }

    #[test]
    fn files_changed_outside_the_editor_refresh_the_index() {
        let host = setup();
        let mut plugin = Laravel::default();
        let text = "<?php\nroute('');\n";
        let d = doc("app/X.php", text);
        let before = plugin.completion(&d, at(text, "('", 2), None);
        assert!(!labels(&before).contains(&"brand.new"));
        host.add_file(
            "routes/web.php",
            &format!("{ROUTES}Route::get('/new', fn () => 1)->name('brand.new');\n"),
        );
        let stale = plugin.completion(&d, at(text, "('", 2), None);
        assert!(!labels(&stale).contains(&"brand.new"));
        plugin.handle_events(
            &[Event::FilesChanged(openidea_plugin::FilesChanged {
                created: Vec::new(),
                modified: vec!["/project/routes/web.php".into()],
                removed: Vec::new(),
            })],
            &host.invocation(),
        );
        let after = plugin.completion(&d, at(text, "('", 2), None);
        assert!(
            labels(&after).contains(&"brand.new"),
            "{:?}",
            labels(&after)
        );
        let many: Vec<String> = (0..MANY_CHANGES + 1)
            .map(|i| format!("/project/app/F{i}.php"))
            .collect();
        plugin.handle_events(
            &[Event::FilesChanged(openidea_plugin::FilesChanged {
                created: many,
                modified: Vec::new(),
                removed: Vec::new(),
            })],
            &host.invocation(),
        );
        let again = plugin.completion(&d, at(text, "('", 2), None);
        assert!(labels(&again).contains(&"brand.new"));
    }

    #[test]
    fn a_picked_value_replaces_the_whole_string() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let text = "<?php\nreturn redirect()->route('invalid-value');\n";
        let items = plugin.completion(&doc("app/X.php", text), at(text, "invalid", 0), None);
        assert!(!items.is_empty());
        let range = items[0].replace.unwrap();
        assert_eq!(range.start, at(text, "invalid", 0));
        assert_eq!(range.end, at(text, "invalid-value'", 13));
        let rules = "<?php\n$request->validate(['a' => 'required|strin|max:3']);\n";
        let items = plugin.completion(&doc("app/X.php", rules), at(rules, "strin", 2), None);
        let range = items[0].replace.unwrap();
        assert_eq!(range.start, at(rules, "strin", 0));
        assert_eq!(range.end, at(rules, "strin|", 5));
    }

    #[test]
    fn ctrl_click_on_a_cache_key_opens_the_cache_tab_filtered() {
        use openidea_plugin::views::{ViewEvent, Views};
        let host = setup();
        let mut plugin = Laravel::default();
        let text = "<?php\n$total = Cache::get('orders.total');\n";
        let d = doc("app/X.php", text);
        assert!(plugin.definition(&d, at(text, "orders", 2)).is_empty());
        host.add_file(
            "storage/openidea/hub/cache.laravel-hub",
            "{\"section\":\"cache\"}\n",
        );
        let found = plugin.definition(&d, at(text, "orders", 2));
        assert_eq!(
            found[0].path,
            "/project/storage/openidea/hub/cache.laravel-hub"
        );
        host.open_editor(
            "/project/storage/openidea/hub/cache.laravel-hub",
            "plaintext",
            "{\"section\":\"cache\"}\n",
            &[],
        );
        plugin
            .handle(
                "hub",
                ViewEvent::Message(r#"{"type":"hello","instance":"tab1"}"#.into()),
                &host.invocation(),
            )
            .unwrap();
        let (view, message) = host.view_messages().pop().unwrap();
        let message: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(view, "hub");
        assert_eq!(message["type"], "navigate");
        assert_eq!(message["id"], "cache");
        assert_eq!(message["cacheKey"], "orders.total");
        assert_eq!(message["instance"], "tab1");
        plugin
            .handle(
                "hub",
                ViewEvent::Message(r#"{"type":"hello","instance":"tab2"}"#.into()),
                &host.invocation(),
            )
            .unwrap();
        let (_, again) = host.view_messages().pop().unwrap();
        assert!(!again.contains("cacheKey"), "the filter is used once");
        host.add_file("storage/openidea/hub/sections", &hubtabs::wanted());
        plugin
            .handle(
                "laravel",
                ViewEvent::Message(
                    r#"{"type":"openeditor","id":"routes","instance":"side"}"#.into(),
                ),
                &host.invocation(),
            )
            .unwrap();
        assert_eq!(
            host.opened_locations().pop().unwrap().0,
            "/project/storage/openidea/hub/routes.laravel-hub"
        );
    }

    #[test]
    fn searches_every_hub_section() {
        use openidea_plugin::views::{ViewEvent, Views};
        let host = setup();
        let mut plugin = Laravel::default();
        let search = |plugin: &mut Laravel, query: &str| {
            plugin
                .handle(
                    "laravel",
                    ViewEvent::Message(
                        serde_json::json!({ "type": "search", "query": query, "instance": "side" })
                            .to_string(),
                    ),
                    &host.invocation(),
                )
                .unwrap();
            let (_, message) = host.view_messages().pop().unwrap();
            serde_json::from_str::<serde_json::Value>(&message).unwrap()
        };
        let found = search(&mut plugin, "users");
        assert_eq!(found["type"], "searchresults");
        assert_eq!(found["instance"], "side");
        let ids: Vec<&str> = found["groups"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|g| g["id"].as_str())
            .collect();
        assert!(
            ids.contains(&"routes") && ids.contains(&"database"),
            "{ids:?}"
        );
        let routes = found["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == "routes")
            .unwrap();
        assert!(routes["rows"][0]["open"]["path"].is_string());
        let none = search(&mut plugin, "zzzz-nothing");
        assert_eq!(none["groups"].as_array().unwrap().len(), 0);
        let short = search(&mut plugin, "u");
        assert_eq!(short["groups"].as_array().unwrap().len(), 0);
    }

    fn install_laravel_10(host: &MockHost) {
        let base = "vendor/laravel/framework/";
        host.add_file(
            &format!("{base}composer.json"),
            r#"{"autoload": {"psr-4": {"Illuminate\\": "src/Illuminate/"}}}"#,
        );
        host.add_file(
            &format!("{base}src/Illuminate/Validation/Concerns/ValidatesAttributes.php"),
            "<?php\nnamespace Illuminate\\Validation\\Concerns;\n\ntrait ValidatesAttributes\n{\n    public function validateRequired($attribute, $value) {}\n    public function validateString($attribute, $value) {}\n    public function validateExists($attribute, $value, $parameters) {}\n}\n",
        );
        host.add_file(
            &format!("{base}src/Illuminate/Database/Eloquent/Concerns/HasAttributes.php"),
            "<?php\nnamespace Illuminate\\Database\\Eloquent\\Concerns;\n\ntrait HasAttributes\n{\n    protected static $primitiveCastTypes = ['array', 'datetime'];\n}\n",
        );
        host.add_file(
            &format!("{base}src/Illuminate/View/Compilers/BladeCompiler.php"),
            "<?php\nnamespace Illuminate\\View\\Compilers;\n\nclass BladeCompiler\n{\n    use Concerns\\CompilesLoops;\n}\n",
        );
        host.add_file(
            &format!("{base}src/Illuminate/View/Compilers/Concerns/CompilesLoops.php"),
            "<?php\nnamespace Illuminate\\View\\Compilers\\Concerns;\n\ntrait CompilesLoops\n{\n    protected function compileForeach($expression) {}\n    protected function compileEndforeach() {}\n}\n",
        );
    }

    #[test]
    fn follows_the_installed_framework_version() {
        let host = setup();
        install_laravel_10(&host);
        let mut plugin = Laravel::default();
        let text = "<?php\n$request->validate(['email' => 'required|hex_color', 'name' => 's']);\n";
        let d = doc("app/X.php", text);
        let before = plugin.completion(&d, at(text, "'s'", 2), None);
        assert!(labels(&before).contains(&"sometimes"));
        plugin.handle_events(
            &[Event::FileOpened("/project/app/X.php".into())],
            &host.invocation(),
        );
        let rules = plugin.completion(&d, at(text, "'s'", 2), None);
        assert_eq!(labels(&rules), ["exists", "string"]);
        let problems = plugin.inspect("/project/app/X.php", text);
        let messages: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
        assert!(messages.contains(&"Validation rule 'hex_color' not found"));
        assert!(!messages.iter().any(|m| m.contains("'required'")));
        let defs = plugin.definition(&d, at(text, "required|", 2));
        assert!(
            defs.iter()
                .any(|l| l.path.ends_with("ValidatesAttributes.php")),
            "{defs:?}"
        );
        let model = "<?php\nclass User extends Authenticatable\n{\n    protected $cas\n}\n";
        let members = plugin.completion(
            &doc("app/Models/User.php", model),
            at(model, "$cas\n", 4),
            None,
        );
        assert_eq!(labels(&members), ["$casts"]);
        assert!(members[0].insert_text.starts_with("protected $casts = "));
        let blade = "<div>\n    @fore\n</div>\n";
        let directives = plugin.completion(
            &doc("resources/views/home.blade.php", blade),
            at(blade, "@fore", 5),
            None,
        );
        assert_eq!(
            labels(&directives),
            ["@foreach", "@endforeach", "@foreach … @end"]
        );
        let casts = "<?php\nclass Post extends Model\n{\n    protected $casts = ['published_at' => ''];\n}\n";
        let types = plugin.completion(
            &doc("app/Models/Post.php", casts),
            at(casts, "=> '", 4),
            None,
        );
        let types = labels(&types);
        assert!(types.contains(&"datetime") && types.contains(&"array"));
        assert!(!types.contains(&"hashed") && !types.contains(&"encrypted"));
    }

    #[test]
    fn completes_columns_for_new_array_keys() {
        let _host = setup();
        let text = "<?php
return Post::query()->create([
    'title' => ,
    ''
]);
";
        let items = Laravel::default().completion(
            &doc("app/X.php", text),
            at(
                text, "''
", 1,
            ),
            None,
        );
        assert_eq!(labels(&items), ["title", "id"]);
    }

    #[test]
    fn completes_cast_types_and_their_arguments() {
        let _host = setup();
        let text = "<?php\nnamespace App\\Models;\n\nclass Post extends Model\n{\n    protected function casts(): array\n    {\n        return [\n            'title' => 'bool',\n            'body' => 'decimal:',\n            'published_at' => 'datetime:',\n            'secret' => 'encrypted:',\n            '' => 'string',\n        ];\n    }\n}\n";
        let d = doc("app/Models/Post.php", text);
        let mut plugin = Laravel::default();
        let types = plugin.completion(&d, at(text, "'bool'", 1), None);
        let types = labels(&types);
        assert!(
            types.contains(&"boolean")
                && types.contains(&"datetime")
                && types.contains(&"encrypted:array")
                && !types.contains(&"title"),
            "{types:?}"
        );
        let decimals = plugin.completion(&d, at(text, "decimal:", 8), None);
        let decimals = labels(&decimals);
        assert!(decimals.contains(&"2"), "{decimals:?}");
        let formats = plugin.completion(&d, at(text, "datetime:", 9), None);
        let formats = labels(&formats);
        assert!(formats.contains(&"Y-m-d H:i:s"), "{formats:?}");
        let encrypted = plugin.completion(&d, at(text, "encrypted:", 10), None);
        let encrypted = labels(&encrypted);
        assert!(encrypted.contains(&"collection"), "{encrypted:?}");
        let keys = plugin.completion(&d, at(text, "'' =>", 1), None);
        let keys = labels(&keys);
        assert!(keys.contains(&"title"), "{keys:?}");
    }

    #[test]
    fn completes_env_keys_and_values() {
        let _host = setup();
        let mut plugin = Laravel::default();
        let text = "APP_NAME=Laravel\nFILESYSTEM_DISK=\nAPP_DEBUG=t\nAPP_\n";
        let d = doc(".env.example", text);
        let disks = plugin.completion(&d, at(text, "DISK=", 5), None);
        assert_eq!(labels(&disks), ["local", "s3"]);
        let debug = plugin.completion(&d, at(text, "DEBUG=t", 7), None);
        assert_eq!(labels(&debug), ["true"]);
        let keys = plugin.completion(&d, at(text, "APP_\n", 4), None);
        assert!(labels(&keys).contains(&"APP_ENV"));
        assert!(!labels(&keys).contains(&"APP_NAME"));
        assert_eq!(
            keys.iter()
                .find(|i| i.label == "APP_ENV")
                .unwrap()
                .insert_text,
            "APP_ENV="
        );
        let defs = plugin.definition(&d, at(text, "APP_NAME", 2));
        assert_eq!(defs[0].path, "/project/config/app.php");
    }

    #[test]
    fn ignores_projects_that_are_not_laravel() {
        let host = MockHost::install();
        host.add_file("routes/web.php", ROUTES);
        let text = "<?php\nroute('');\n";
        assert!(
            Laravel::default()
                .completion(&doc("x.php", text), at(text, "('", 2), None)
                .is_empty()
        );
    }

    #[test]
    fn finds_related_livewire_and_component_files() {
        assert_eq!(
            related_candidates("app/Livewire/Admin/UserTable.php")[0],
            "resources/views/livewire/admin/user-table.blade.php"
        );
        assert!(
            related_candidates("resources/views/livewire/admin/user-table.blade.php")
                .contains(&"app/Livewire/Admin/UserTable.php".to_owned())
        );
        assert_eq!(
            related_candidates("app/View/Components/Forms/TextInput.php"),
            ["resources/views/components/forms/text-input.blade.php"]
        );
        assert!(related_candidates("app/Models/User.php").is_empty());
    }

    #[test]
    fn a_healthy_project_logs_nothing() {
        let host = setup();
        let mut plugin = Laravel::default();
        plugin.handle_events(
            &[
                Event::FileOpened("/project/routes/web.php".into()),
                Event::FileSaved("/project/app/Models/User.php".into()),
            ],
            &host.invocation(),
        );
        plugin
            .run_command("refresh-index", &host.invocation(), None)
            .unwrap();
        assert_eq!(host.logs(), Vec::new());
    }

    #[test]
    fn failures_are_logged_as_warnings_and_errors() {
        let host = setup().with_capabilities(&["project.read", "process:php"]);
        host.set_setting("phpstanOnSave", "true");
        host.add_file(
            "phpstan.neon",
            "parameters:
    level: 5
",
        );
        host.add_file("vendor/bin/phpstan", "");
        host.on_process("php", |_, _| {
            Ok(openidea_plugin::ProcessOutput {
                status: Some(255),
                stdout: String::new(),
                stderr: "PHP Fatal error: Allowed memory size
"
                .into(),
            })
        });
        let mut plugin = Laravel::default();
        plugin.handle_events(
            &[Event::FileSaved("/project/app/Models/User.php".into())],
            &host.invocation(),
        );
        let err = plugin
            .run_command("nope", &host.invocation(), None)
            .unwrap_err();
        let logs = host.logs();
        assert!(
            logs.contains(&(
                openidea_plugin::LogLevel::Error,
                "phpstan on save: no results for app/Models/User.php: exit code 255
PHP Fatal error: Allowed memory size"
                    .into()
            )),
            "{logs:?}"
        );
        assert!(
            logs.contains(&(
                openidea_plugin::LogLevel::Warn,
                format!("command nope: {err}")
            )),
            "{logs:?}"
        );
        assert!(logs.iter().all(|(level, _)| matches!(
            level,
            openidea_plugin::LogLevel::Warn | openidea_plugin::LogLevel::Error
        )));
    }
}
