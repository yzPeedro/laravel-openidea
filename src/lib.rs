mod artisan;
mod blade_syntax;
mod casts;
mod checks;
mod completions;
mod devtools;
mod directives;
mod dotenv;
mod eloquent;
mod formrequest;
mod hierarchy;
mod index;
mod inspect;
mod inspections;
mod logs;
mod members;
mod modeldoc;
mod panels;
mod pest;
mod php;
mod project;
mod queries;
mod querysql;
mod refs;
mod renames;
mod resources;
mod runner;
mod sections;
mod toolconfig;
mod tools;

use index::{Cache, Entry, RefKind, is_model, parse_class};
use openidea_plugin::providers::{CodeAction, CompletionItem, Document, LineMarker, Providers};
use openidea_plugin::text::{offset, position};
use openidea_plugin::{
    Context, Diagnostic, Error, Event, EventKind, Invocation, Location, LogLevel, Position,
    Project, Range, Severity, StatusItem, TextEdit, host,
};
use php::{StringArg, is_blade};
use project::{Lines, join, project_of};
use refs::{Scope, Segment, Want};
use std::collections::{BTreeMap, BTreeSet};

const MAX_COMPLETIONS: usize = 500;
const STATUS_ID: &str = "laravel";

type References = Vec<(Segment, StringArg)>;

#[derive(Default)]
pub struct Laravel {
    cache: Cache,
    projects: BTreeMap<String, bool>,
    commands: BTreeMap<String, std::rc::Rc<Vec<artisan::Command>>>,
    symbols: BTreeMap<String, std::rc::Rc<Vec<hierarchy::ProjectSymbol>>>,
    saved: Vec<renames::Saved>,
    originals: BTreeMap<String, String>,
    analysis: BTreeMap<String, Vec<Diagnostic>>,
    warned: BTreeSet<String>,
    pest: BTreeMap<String, pest::Names>,
}

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
    fn project_symbols(
        &mut self,
        project: &Project,
        root: &str,
    ) -> std::rc::Rc<Vec<hierarchy::ProjectSymbol>> {
        self.symbols
            .entry(root.to_owned())
            .or_insert_with(|| {
                std::rc::Rc::new(hierarchy::project_symbols(&resources::scan(project, root)))
            })
            .clone()
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
                    host::log(LogLevel::Info, &format!("Laravel project at {root}"));
                    show_status(&project);
                }
                known
            }
        };
        laravel.then_some((project, root))
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
        let model = members::Model {
            table: &table,
            columns: members::columns_of(&column_entries, &table),
            models,
            foreign,
            relations,
        };
        members::items(&model, ctx, at, position(text, ctx.start))
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
                let (kind, entries) = match dotenv::values_for(&key) {
                    Some(dotenv::Values::Kind(kind)) => {
                        (kind, self.cache.get(kind, project, root).to_vec())
                    }
                    Some(dotenv::Values::Static(values)) => (
                        RefKind::Rule,
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
                            .collect(),
                    ),
                    Some(dotenv::Values::Boolean) => (
                        RefKind::Rule,
                        ["true", "false"]
                            .iter()
                            .map(|v| Entry::new(*v, None, "", Range::default()))
                            .collect(),
                    ),
                    Some(dotenv::Values::Locales) => {
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
                        locales.extend(
                            ["en", "en_US", "pt_BR", "es", "fr", "de", "it"].map(str::to_owned),
                        );
                        (
                            RefKind::Rule,
                            locales
                                .into_iter()
                                .map(|l| Entry::new(l, Some("locale".into()), "", Range::default()))
                                .collect(),
                        )
                    }
                    None => return Vec::new(),
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
        (text, cursor): (&str, usize),
        base: &eloquent::Base,
        path: &[String],
        depth: u8,
    ) -> Option<(String, bool)> {
        if depth > 4 {
            return None;
        }
        let models: BTreeSet<String> = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .map(|e| e.key.clone())
            .collect();
        let (mut current, mut many) = match base {
            eloquent::Base::This => {
                let scope = scope_of(text);
                (scope.class.filter(|_| scope.model)?, false)
            }
            eloquent::Base::AuthUser => ("User".to_owned(), false),
            eloquent::Base::Var(var) => match eloquent::origin(text, cursor, var)? {
                eloquent::Origin::Class { name, many } => (name, many),
                eloquent::Origin::Chain {
                    base,
                    path,
                    element,
                } => {
                    let (model, many) =
                        self.model_of(project, root, (text, cursor), &base, &path, depth + 1)?;
                    if element && !many {
                        return None;
                    }
                    (model, many && !element)
                }
            },
        };
        if !models.contains(&current) {
            return None;
        }
        let relations = self.cache.get(RefKind::Relation, project, root);
        for step in path {
            if many {
                return None;
            }
            let entry = relations
                .iter()
                .find(|e| e.key == format!("{current}.{step}"))?;
            let detail = entry.detail.as_deref()?;
            many = eloquent::is_many(detail.split('(').next().unwrap_or(detail));
            current = relation_target(Some(detail))?;
        }
        Some((current, many))
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
        project.read_file(&path).ok()
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
        let typed = &doc.text[access.start..cursor];
        let replace = Range::new(position(&doc.text, access.start), at);
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
            Want::Static(values) => {
                let entries = values
                    .iter()
                    .map(|(k, d)| {
                        Entry::new(
                            *k,
                            (!d.is_empty()).then(|| (*d).to_owned()),
                            "",
                            Range::default(),
                        )
                    })
                    .collect();
                (RefKind::Rule, entries)
            }
            Want::SchemaProperties(call_start) => {
                (RefKind::Column, schema_properties(doc, *call_start))
            }
            Want::PathParams(before) => (RefKind::Column, path_params(doc, *before)),
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
        let Ok(text) = project.read_file(path) else {
            return;
        };
        let mut items = self.inspect(path, &text);
        if let Some(found) = self.analysis.get(path) {
            items.extend(found.iter().cloned());
        }
        let _ = host::publish_diagnostics(path, &items);
    }

    fn artisan(&self, ctx: &Invocation, args: &[&str]) -> Result<Option<String>, Error> {
        let project = ctx.project().ok_or("open a Laravel project first")?;
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let out = project.run_process("php", &args, None)?;
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
        if let Some((file, rest)) = key.split_once('.')
            && !key.contains(' ')
        {
            let relative = format!("lang/{locale}/{file}.php");
            if let Ok(text) = project.read_file(&relative)
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
        }
        let relative = format!("lang/{locale}.json");
        if let Ok(text) = project.read_file(&relative)
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
        if php::open_arg_at(&doc.text, blade, cursor).is_none()
            && let Some(found) = self.eloquent_members(&project, &root, doc, cursor, at)
            && !found.is_empty()
        {
            return found;
        }
        if blade && let Some(start) = directives::typed_at(&doc.text, cursor) {
            let mut entries: Vec<Entry> = directives::BUILTIN
                .iter()
                .map(|(name, detail)| {
                    Entry::new(*name, Some((*detail).to_owned()), "", Range::default())
                })
                .collect();
            entries.extend(
                self.cache
                    .get(RefKind::Directive, &project, &root)
                    .iter()
                    .cloned(),
            );
            let typed = &doc.text[start + 1..cursor];
            let replace = Range::new(position(&doc.text, start), at);
            return items(RefKind::Directive, &entries, typed, replace)
                .into_iter()
                .map(|mut item| {
                    item.label = format!("@{}", item.label);
                    item.insert_text = item.label.clone();
                    item
                })
                .collect();
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
            let symbols = self.project_symbols(&project, &root);
            let composer = project.read_file("composer.json").unwrap_or_default();
            return hierarchy::items(
                slot,
                &doc.text,
                project::rel(&root, &doc.path),
                start,
                at,
                &composer,
                &symbols,
            );
        }
        if !blade
            && php::open_arg_at(&doc.text, false, cursor).is_none()
            && let Some(ctx) = members::context(&doc.text, cursor)
        {
            let scope = scope_of(&doc.text);
            if let (true, Some(class)) = (scope.model, scope.class.as_deref()) {
                return self.member_items(&project, &root, class, &doc.text, &ctx, at);
            }
        }
        let Some(arg) = php::open_arg_at(&doc.text, blade, cursor) else {
            return Vec::new();
        };
        let scope = scope_of(&doc.text);
        let within = cursor - arg.start;
        let Some(seg) = refs::segment(&arg, within, &scope) else {
            return Vec::new();
        };
        let (kind, entries) = self.resolve(&seg.want, &project, &root, doc);
        let start = (arg.start + seg.start).min(cursor);
        items(
            kind,
            &entries,
            &doc.text[start..cursor],
            Range::new(position(&doc.text, start), at),
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
                let Some(arg) = php::arg_at(&doc.text, blade, cursor) else {
                    return Vec::new();
                };
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
        out
    }

    fn resolve_code_action(
        &mut self,
        doc: &Document,
        action: CodeAction,
    ) -> Result<CodeAction, Error> {
        self.resolve_pending(doc, action)
    }

    fn code_actions(&mut self, doc: &Document, selection: Range) -> Vec<CodeAction> {
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

impl openidea_plugin::Plugin for Laravel {
    fn activate(&mut self, _ctx: &Context) -> Result<(), Error> {
        self.cache.locale = host::setting_string("locale")
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "en".into());
        let _ = host::subscribe(EventKind::FileOpened);
        let _ = host::subscribe(EventKind::FileSaved);
        let _ = host::subscribe(EventKind::VcsChanged);
        for project in host::projects().unwrap_or_default() {
            if let Ok(root) = project.root()
                && let Some((project, root)) = self.laravel_project(&root)
            {
                self.cache.warm(&project, &root);
                self.project_symbols(&project, &root);
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
        match id {
            "refresh-index" => {
                self.cache.clear();
                self.projects.clear();
                let project = ctx.project().ok_or("open a Laravel project first")?;
                let root = project.root()?;
                if !project::is_laravel(&project) {
                    return Err("this project is not a Laravel application".into());
                }
                show_status(&project);
                let counts: Vec<String> = RefKind::ALL
                    .iter()
                    .map(|k| (self.cache.get(*k, &project, &root).len(), k.label()))
                    .filter(|(n, _)| *n > 0)
                    .map(|(n, label)| format!("{n} {label}"))
                    .collect();
                Ok(Some(format!("Indexed {}", counts.join(", "))))
            }
            "related-file" => self.related_file(ctx),
            "artisan-optimize-clear" => self.artisan(ctx, &["artisan", "optimize:clear"]),
            "artisan-migrate" => self.artisan(ctx, &["artisan", "migrate"]),
            "artisan-tinker" => {
                host::open_terminal("Tinker", "php", &["artisan".into(), "tinker".into()])?;
                Ok(None)
            }
            "copy-query-sql" => self.copy_query_sql(ctx),
            "run-in-tinker" => self.run_selection_in_tinker(ctx),
            "artisan-serve" => {
                host::open_terminal("artisan serve", "php", &["artisan".into(), "serve".into()])?;
                Ok(None)
            }
            _ => Err(Error::NotFound(format!("command {id}"))),
        }
    }

    fn handle_events(&mut self, events: &[Event], _ctx: &Invocation) {
        let mut paths = BTreeSet::new();
        for event in events {
            match event {
                Event::FileSaved(path) => {
                    if let Some(project) = project_of(path)
                        && let Ok(root) = project.root()
                    {
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
                        if path.ends_with(".php") {
                            self.symbols.remove(&root);
                        }
                    }
                    paths.insert(path.clone());
                }
                Event::FileOpened(path) => {
                    if let Some(project) = project_of(path)
                        && let Ok(root) = project.root()
                        && let Ok(text) = project.read_file(path)
                    {
                        self.remember_original(&root, path, &text, true);
                    }
                    paths.insert(path.clone());
                }
                Event::VcsChanged(repo) => {
                    self.cache.clear();
                    self.symbols.clear();
                    paths.insert(repo.clone());
                }
                _ => {}
            }
        }
        for path in paths {
            if let Some((project, root)) = self.laravel_project(&path) {
                self.cache.warm(&project, &root);
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
            "<?php\nnamespace App\\Models;\n\nclass Order extends Model\n{\n    public function user()\n    {\n        return $this->belongsTo(User::class);\n    }\n\n    public function lines()\n    {\n        return $this->hasMany(Post::class);\n    }\n}\n",
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
        assert!(
            plugin
                .completion(&d, at(text, "$order->lines->", 15), None)
                .is_empty()
        );

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
        assert!(labels(&rel).is_empty(), "author() already exists");
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
            ["@foreach", "@endforeach", "@forelse", "@endforelse"]
        );
        assert_eq!(items[0].replace.unwrap().start, at(text, "@fore", 0));
        let custom = plugin.completion(&d, at(text, "@adm", 4), None);
        assert_eq!(
            labels(&custom),
            ["@admin", "@elseadmin", "@unlessadmin", "@endadmin"]
        );
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
}
