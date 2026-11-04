use crate::index::{Cache, RefKind};
use crate::project::rel;
use crate::resources::{
    self, Scan, Symbol, array_pairs, bindings, class_refs, folder, listen_property, listeners,
    observers, role, short,
};
use openidea_plugin::Project;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub path: String,
    pub line: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<String>,
    pub open: Option<Target>,
}

impl Row {
    pub fn at(cells: Vec<String>, path: &str, line: u32) -> Self {
        Self {
            cells,
            open: (!path.is_empty()).then(|| Target {
                path: path.to_owned(),
                line: line.max(1),
            }),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Facet {
    pub column: &'static str,
    pub split: bool,
}

pub const fn facet(column: &'static str) -> Facet {
    Facet {
        column,
        split: false,
    }
}

pub const fn split(column: &'static str) -> Facet {
    Facet {
        column,
        split: true,
    }
}

pub struct Table {
    pub columns: Vec<&'static str>,
    pub rows: Vec<Row>,
    pub facets: Vec<Facet>,
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "—".into()
    } else {
        items.join(", ")
    }
}

fn yes(b: bool) -> String {
    if b { "yes" } else { "no" }.into()
}

fn of_role<'a>(scan: &'a Scan, wanted: &'a [&str]) -> impl Iterator<Item = &'a Symbol> {
    scan.symbols
        .iter()
        .filter(move |s| s.keyword == "class" && wanted.contains(&role(s)))
}

fn sym_row(s: &Symbol, mut cells: Vec<String>) -> Row {
    cells.push(folder(&s.path));
    Row::at(cells, &s.path, s.line)
}

fn public_actions(s: &Symbol) -> Vec<String> {
    s.methods
        .iter()
        .filter(|m| m.public && !m.name.starts_with("__") && m.name != "middleware")
        .map(|m| m.name.clone())
        .collect()
}

fn method_line(s: &Symbol, name: &str) -> u32 {
    s.methods
        .iter()
        .find(|m| m.name == name)
        .map_or(s.line, |m| m.line)
}

pub fn event_listeners(scan: &Scan, project: &Project, root: &str) -> Vec<resources::Listener> {
    let mut all = listeners(scan);
    all.extend(listen_property(project, root));
    all
}

pub fn build(
    id: &str,
    scan: &Scan,
    project: &Project,
    root: &str,
    cache: &mut Cache,
) -> Option<Table> {
    let table = |columns: Vec<&'static str>, rows: Vec<Row>, facets: Vec<Facet>| {
        Some(Table {
            columns,
            rows,
            facets,
        })
    };
    match id {
        "classes" => {
            let rows = scan
                .symbols
                .iter()
                .map(|s| {
                    let mut kind = role(s).to_owned();
                    if s.is_abstract {
                        kind.push_str(" (abstract)");
                    }
                    sym_row(
                        s,
                        vec![
                            s.name.clone(),
                            kind,
                            list(&s.extends),
                            list(&s.implements),
                            list(&s.traits),
                        ],
                    )
                })
                .collect();
            table(
                vec!["Class", "Kind", "Extends", "Implements", "Traits", "Folder"],
                rows,
                vec![
                    facet("Kind"),
                    facet("Folder"),
                    split("Implements"),
                    split("Traits"),
                ],
            )
        }
        "controllers" => {
            let rows = of_role(scan, &["Controller"])
                .filter(|s| !s.is_abstract && s.name != "Controller")
                .map(|s| {
                    let actions = public_actions(s);
                    let has = |n: &str| s.methods.iter().any(|m| m.name == n);
                    let kind = if has("__invoke") {
                        "invokable"
                    } else if ["index", "store", "show", "update", "destroy"]
                        .iter()
                        .filter(|n| has(n))
                        .count()
                        >= 3
                    {
                        "resource"
                    } else {
                        "controller"
                    };
                    sym_row(
                        s,
                        vec![
                            s.name.clone(),
                            kind.into(),
                            actions.len().to_string(),
                            list(&actions),
                        ],
                    )
                })
                .collect();
            table(
                vec!["Controller", "Kind", "Count", "Actions", "Folder"],
                rows,
                vec![facet("Kind"), facet("Folder")],
            )
        }
        "requests" => {
            let rows = of_role(scan, &["Form request"])
                .filter(|s| !s.is_abstract)
                .map(|s| {
                    let fields: Vec<String> = s
                        .methods
                        .iter()
                        .find(|m| m.name == "rules")
                        .map(|m| {
                            array_pairs(&m.body)
                                .into_iter()
                                .map(|(k, _)| k.trim_matches(['\'', '"']).to_owned())
                                .filter(|k| {
                                    !k.is_empty()
                                        && k.chars()
                                            .all(|c| c.is_alphanumeric() || "_.*-".contains(c))
                                })
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .collect()
                        })
                        .unwrap_or_default();
                    let authorizes = s.methods.iter().any(|m| m.name == "authorize");
                    sym_row(
                        s,
                        vec![
                            s.name.clone(),
                            fields.len().to_string(),
                            list(&fields),
                            yes(authorizes),
                        ],
                    )
                })
                .collect();
            table(
                vec!["Request", "Count", "Fields", "Authorizes", "Folder"],
                rows,
                vec![facet("Authorizes"), facet("Folder")],
            )
        }
        "api-resources" => {
            let rows = of_role(scan, &["API resource"])
                .map(|s| {
                    let collection = s.extends.iter().any(|e| e == "ResourceCollection")
                        || s.name.ends_with("Collection");
                    let model = s
                        .name
                        .trim_end_matches("Collection")
                        .trim_end_matches("Resource")
                        .to_owned();
                    sym_row(
                        s,
                        vec![
                            s.name.clone(),
                            if collection { "collection" } else { "resource" }.into(),
                            model,
                        ],
                    )
                })
                .collect();
            table(
                vec!["Resource", "Kind", "Wraps", "Folder"],
                rows,
                vec![facet("Kind"), facet("Folder")],
            )
        }
        "enums" => {
            let rows = scan
                .symbols
                .iter()
                .filter(|s| s.keyword == "enum")
                .map(|s| {
                    sym_row(
                        s,
                        vec![
                            s.name.clone(),
                            s.backing.clone().unwrap_or_else(|| "pure".into()),
                            s.cases.len().to_string(),
                            list(&s.cases),
                            list(&s.implements),
                        ],
                    )
                })
                .collect();
            table(
                vec!["Enum", "Type", "Count", "Cases", "Implements", "Folder"],
                rows,
                vec![facet("Type"), split("Implements"), facet("Folder")],
            )
        }
        "casts" | "rules" => {
            let wanted: &[&str] = if id == "casts" { &["Cast"] } else { &["Rule"] };
            let rows = of_role(scan, wanted)
                .map(|s| sym_row(s, vec![s.name.clone(), list(&s.implements)]))
                .collect();
            table(
                vec![
                    if id == "casts" { "Cast" } else { "Rule" },
                    "Implements",
                    "Folder",
                ],
                rows,
                vec![split("Implements"), facet("Folder")],
            )
        }
        "scopes" => {
            let mut rows: Vec<Row> = cache
                .get(RefKind::Scope, project, root)
                .iter()
                .map(|e| {
                    let (model, scope) = e.key.split_once('.').unwrap_or(("", e.key.as_str()));
                    Row::at(
                        vec![
                            scope.into(),
                            "local".into(),
                            model.into(),
                            rel(root, &e.location.path).into(),
                        ],
                        rel(root, &e.location.path),
                        e.location.range.start.line + 1,
                    )
                })
                .collect();
            let mut applied: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for s in &scan.symbols {
                for a in s.attributes.iter().filter(|a| a.name == "ScopedBy") {
                    for c in class_refs(&a.args) {
                        applied.entry(c).or_default().insert(s.name.clone());
                    }
                }
            }
            for c in scan.calls.iter().filter(|c| c.method == "addGlobalScope") {
                let classes: Vec<String> =
                    c.args.first().map(|a| class_refs(a)).unwrap_or_default();
                if classes.is_empty() {
                    let name = c
                        .args
                        .first()
                        .map(|a| a.trim_matches(['\'', '"']).to_owned())
                        .filter(|a| !a.starts_with("function") && !a.starts_with("fn"))
                        .unwrap_or_else(|| "closure".into());
                    rows.push(Row::at(
                        vec![
                            name,
                            "global (closure)".into(),
                            c.owner.clone(),
                            c.path.clone(),
                        ],
                        &c.path,
                        c.line,
                    ));
                }
                for class in classes {
                    applied.entry(class).or_default().insert(c.owner.clone());
                }
            }
            for s in of_role(scan, &["Scope"]) {
                let models: Vec<String> = applied
                    .remove(&s.name)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                rows.push(Row::at(
                    vec![
                        s.name.clone(),
                        "global".into(),
                        list(&models),
                        s.path.clone(),
                    ],
                    &s.path,
                    s.line,
                ));
            }
            table(
                vec!["Scope", "Kind", "Model", "File"],
                rows,
                vec![facet("Kind"), split("Model")],
            )
        }
        "observers" => {
            let rows = observers(scan)
                .into_iter()
                .map(|o| {
                    Row::at(
                        vec![
                            o.observer,
                            list(&o.models),
                            list(&o.hooks),
                            o.via,
                            o.path.clone(),
                        ],
                        &o.path,
                        o.line,
                    )
                })
                .collect();
            table(
                vec!["Observer", "Model", "Hooks", "Registered via", "File"],
                rows,
                vec![split("Model"), split("Hooks"), facet("Registered via")],
            )
        }
        "listeners" => {
            let rows = event_listeners(scan, project, root)
                .into_iter()
                .map(|l| {
                    let name =
                        if l.method.is_empty() || l.method == "handle" || l.method == "__invoke" {
                            l.listener
                        } else {
                            format!("{}@{}", l.listener, l.method)
                        };
                    Row::at(
                        vec![name, list(&l.events), yes(l.queued), l.via, l.path.clone()],
                        &l.path,
                        l.line,
                    )
                })
                .collect();
            table(
                vec!["Listener", "Listens to", "Queued", "Registered via", "File"],
                rows,
                vec![
                    split("Listens to"),
                    facet("Queued"),
                    facet("Registered via"),
                ],
            )
        }
        "events" => {
            let mut by_event: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for l in event_listeners(scan, project, root) {
                for e in &l.events {
                    by_event
                        .entry(short(e).to_owned())
                        .or_default()
                        .insert(l.listener.clone());
                }
            }
            let mut rows = Vec::new();
            for s in scan
                .symbols
                .iter()
                .filter(|s| s.keyword == "class" && role(s) == "Event")
            {
                let names: Vec<String> = by_event
                    .remove(&s.name)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                let broadcast = s
                    .implements
                    .iter()
                    .any(|i| i.starts_with("ShouldBroadcast"));
                rows.push(sym_row(
                    s,
                    vec![
                        s.name.clone(),
                        if broadcast { "app · broadcast" } else { "app" }.into(),
                        names.len().to_string(),
                        list(&names),
                    ],
                ));
            }
            for (event, names) in by_event {
                let names: Vec<String> = names.into_iter().collect();
                rows.push(Row::at(
                    vec![
                        event,
                        "framework / package".into(),
                        names.len().to_string(),
                        list(&names),
                        String::new(),
                    ],
                    "",
                    0,
                ));
            }
            table(
                vec!["Event", "Kind", "Count", "Listeners", "Folder"],
                rows,
                vec![facet("Kind"), split("Listeners")],
            )
        }
        "providers" => {
            let registered: String = ["bootstrap/providers.php", "config/app.php"]
                .iter()
                .filter_map(|f| project.read_file(f).ok())
                .collect();
            let binds = bindings(scan);
            let rows = of_role(scan, &["Provider"])
                .map(|s| {
                    let count = binds.iter().filter(|b| b.path == s.path).count();
                    let hooks: Vec<String> = s
                        .methods
                        .iter()
                        .filter(|m| m.name == "register" || m.name == "boot")
                        .map(|m| m.name.clone())
                        .collect();
                    sym_row(
                        s,
                        vec![
                            s.name.clone(),
                            yes(registered.contains(&s.name)),
                            count.to_string(),
                            list(&hooks),
                        ],
                    )
                })
                .collect();
            table(
                vec!["Provider", "Registered", "Bindings", "Methods", "Folder"],
                rows,
                vec![facet("Registered")],
            )
        }
        "bindings" => {
            let rows = bindings(scan)
                .into_iter()
                .map(|b| {
                    Row::at(
                        vec![b.abstract_, b.concrete, b.lifetime, b.path.clone()],
                        &b.path,
                        b.line,
                    )
                })
                .collect();
            table(
                vec!["Abstract", "Concrete", "Lifetime", "File"],
                rows,
                vec![facet("Lifetime")],
            )
        }
        "factories" => {
            let rows = of_role(scan, &["Factory", "Seeder"])
                .map(|s| {
                    let kind = role(s);
                    let target = if kind == "Factory" {
                        s.name.trim_end_matches("Factory").to_owned()
                    } else {
                        let calls: Vec<String> = s
                            .methods
                            .iter()
                            .find(|m| m.name == "run")
                            .map(|m| class_refs(&m.body))
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|c| c.ends_with("Seeder"))
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect();
                        list(&calls)
                    };
                    Row::at(
                        vec![s.name.clone(), kind.into(), target, s.path.clone()],
                        &s.path,
                        method_line(
                            s,
                            if kind == "Factory" {
                                "definition"
                            } else {
                                "run"
                            },
                        ),
                    )
                })
                .collect();
            table(
                vec!["Class", "Kind", "Model / uses", "File"],
                rows,
                vec![facet("Kind")],
            )
        }
        "macros" => {
            let rows = scan
                .calls
                .iter()
                .filter(|c| c.method == "macro" || c.method == "mixin")
                .map(|c| {
                    let name = c
                        .args
                        .first()
                        .map(|a| {
                            class_refs(a)
                                .pop()
                                .unwrap_or_else(|| a.trim_matches(['\'', '"']).to_owned())
                        })
                        .unwrap_or_default();
                    Row::at(
                        vec![name, c.class.clone(), c.method.clone(), c.path.clone()],
                        &c.path,
                        c.line,
                    )
                })
                .collect();
            table(
                vec!["Macro", "On", "Kind", "File"],
                rows,
                vec![facet("On"), facet("Kind")],
            )
        }
        "services" => {
            let kinds = [
                (RefKind::DbConnection, "database connection"),
                (RefKind::CacheStore, "cache store"),
                (RefKind::Disk, "filesystem disk"),
                (RefKind::Queue, "queue connection"),
                (RefKind::Mailer, "mailer"),
                (RefKind::LogChannel, "log channel"),
                (RefKind::Guard, "auth guard"),
                (RefKind::PasswordBroker, "password broker"),
                (RefKind::Broadcaster, "broadcaster"),
                (RefKind::Channel, "broadcast channel"),
                (RefKind::RateLimiter, "rate limiter"),
            ];
            let mut rows = Vec::new();
            for (kind, label) in kinds {
                for e in cache.get(kind, project, root).iter() {
                    rows.push(Row::at(
                        vec![
                            e.key.clone(),
                            label.into(),
                            e.detail.clone().unwrap_or_default(),
                        ],
                        rel(root, &e.location.path),
                        e.location.range.start.line + 1,
                    ));
                }
            }
            table(vec!["Name", "Kind", "Detail"], rows, vec![facet("Kind")])
        }
        "frontend" => {
            let mut rows = Vec::new();
            for (kind, label) in [
                (RefKind::Asset, "Vite asset"),
                (RefKind::InertiaPage, "Inertia page"),
                (RefKind::Livewire, "Livewire component"),
                (RefKind::Component, "Blade component"),
            ] {
                for e in cache.get(kind, project, root).iter() {
                    rows.push(Row::at(
                        vec![
                            e.key.clone(),
                            label.into(),
                            e.detail.clone().unwrap_or_default(),
                        ],
                        rel(root, &e.location.path),
                        e.location.range.start.line + 1,
                    ));
                }
            }
            table(vec!["Name", "Kind", "File"], rows, vec![facet("Kind")])
        }
        "policies" => {
            let mut rows = Vec::new();
            for (kind, label) in [
                (RefKind::Ability, "ability"),
                (RefKind::Role, "role"),
                (RefKind::Permission, "permission"),
                (RefKind::TokenAbility, "token ability"),
                (RefKind::Guard, "guard"),
            ] {
                for e in cache.get(kind, project, root).iter() {
                    rows.push(Row::at(
                        vec![
                            e.key.clone(),
                            label.into(),
                            e.detail.clone().unwrap_or_default(),
                        ],
                        rel(root, &e.location.path),
                        e.location.range.start.line + 1,
                    ));
                }
            }
            for s in of_role(scan, &["Policy"]) {
                let abilities = public_actions(s);
                rows.push(sym_row(
                    s,
                    vec![s.name.clone(), "policy".into(), list(&abilities)],
                ));
                rows.last_mut().unwrap().cells.pop();
            }
            table(
                vec!["Name", "Kind", "Defined in / abilities"],
                rows,
                vec![facet("Kind")],
            )
        }
        _ => None,
    }
}

pub fn counts(scan: &Scan, project: &Project, root: &str) -> BTreeMap<&'static str, usize> {
    let mut out = BTreeMap::new();
    let n = |roles: &[&str]| {
        scan.symbols
            .iter()
            .filter(|s| s.keyword == "class" && roles.contains(&role(s)))
            .count()
    };
    out.insert("classes", scan.symbols.len());
    out.insert(
        "controllers",
        of_role(scan, &["Controller"])
            .filter(|s| !s.is_abstract && s.name != "Controller")
            .count(),
    );
    out.insert("requests", n(&["Form request"]));
    out.insert("api-resources", n(&["API resource"]));
    out.insert(
        "enums",
        scan.symbols.iter().filter(|s| s.keyword == "enum").count(),
    );
    out.insert("casts", n(&["Cast"]));
    out.insert("rules", n(&["Rule"]));
    out.insert("observers", observers(scan).len());
    out.insert("listeners", event_listeners(scan, project, root).len());
    out.insert("events", n(&["Event"]));
    out.insert("providers", n(&["Provider"]));
    out.insert("bindings", bindings(scan).len());
    out.insert("factories", n(&["Factory", "Seeder"]));
    out.insert(
        "macros",
        scan.calls
            .iter()
            .filter(|c| c.method == "macro" || c.method == "mixin")
            .count(),
    );
    out
}
