use crate::Laravel;
use crate::audit::Logged;
use crate::index::RefKind;
use crate::php::{self, Kind, is_blade};
use crate::project::{Lines, find, rel};
use crate::refs::{self, Want};
use openidea_plugin::file_actions::{FileAction, FileActions, FileOperation};
use openidea_plugin::providers::{CodeAction, Document};
use openidea_plugin::text::offset;
use openidea_plugin::{Error, Invocation, Location, Project, Range, TextEdit, host};
use serde::{Deserialize, Serialize};

fn text_edits(text: &str, edits: Vec<(usize, usize, String)>) -> Vec<TextEdit> {
    let lines = Lines::new(text);
    edits
        .into_iter()
        .map(|(s, e, t)| TextEdit {
            range: lines.range(text, s, e),
            text: t,
        })
        .collect()
}

fn create_form_request(project: &Project, x: &crate::formrequest::Extraction) -> Result<(), Error> {
    create_file(project, &x.file, &x.content, "the Form Request")
}

pub fn create_file(project: &Project, file: &str, content: &str, what: &str) -> Result<(), Error> {
    let mode = crate::runner::detect(project);
    let args = mode.php_args(&[
        "-r".into(),
        crate::devtools::script(crate::devtools::CREATE_FILE_PHP),
        "--".into(),
        file.to_owned(),
    ]);
    let out = match crate::runner::run(project, &mode, &args, Some(content)) {
        Ok(out) => out,
        Err(Error::Denied(cap)) => {
            return Err(Error::Failed(format!(
                "Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to create {what}."
            )));
        }
        Err(e) => return Err(e),
    };
    if out.status != Some(0) {
        let why = if out.stderr.trim().is_empty() {
            out.stdout.trim()
        } else {
            out.stderr.trim()
        };
        return Err(Error::Failed(format!("Couldn't create {file}: {why}")));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Pending {
    Rename {
        kind: String,
        old: String,
        new: String,
        path: String,
    },
    Views {
        old: String,
        new: String,
    },
    FormRequest {
        at: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteTarget {
    pub method: String,
    pub uri: String,
    pub name: String,
    pub action: String,
    pub file: String,
    pub exists: bool,
}

impl RouteTarget {
    pub fn title(&self) -> String {
        if self.exists {
            format!("Open the feature test ({})", self.file)
        } else {
            format!(
                "Generate a feature test for {} /{} ({})",
                self.method,
                self.uri.trim_start_matches('/'),
                self.file
            )
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub path: String,
    pub kind: RefKind,
    pub old: String,
    pub new: String,
}

pub fn edited(disk: &str, buffer: &str, start: usize, end: usize) -> Option<String> {
    let prefix = buffer.get(..start)?;
    let suffix = buffer.get(end..)?;
    if !disk.starts_with(prefix) || !disk.ends_with(suffix) || disk.len() < start + suffix.len() {
        return None;
    }
    let old = &disk[start..disk.len() - suffix.len()];
    (old != &buffer[start..end] && !old.contains(['\n', '\'', '"'])).then(|| old.to_owned())
}

pub struct Str {
    pub start: usize,
    pub end: usize,
    pub value: String,
}

pub fn string_at(text: &str, cursor: usize) -> Option<Str> {
    let tokens = php::tokenize(text, 0, text.len());
    tokens.iter().find_map(|t| {
        let Kind::Str { value, .. } = &t.kind else {
            return None;
        };
        let quoted = matches!(text.as_bytes().get(t.start), Some(b'\'' | b'"'));
        let start = t.start + usize::from(quoted);
        let end = start + value.len();
        (start <= cursor && cursor <= end && text.get(start..end) == Some(value.as_str())).then(
            || Str {
                start,
                end,
                value: value.clone(),
            },
        )
    })
}

pub fn names_route(text: &str, quote_at: usize) -> bool {
    let before: String = text[..quote_at]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    before.ends_with("->name(") || before.ends_with("?->name(")
}

pub fn lang_file(rel_path: &str) -> Option<(String, Option<String>)> {
    let p = rel_path.replace('\\', "/");
    let rest = p
        .strip_prefix("lang/")
        .or_else(|| p.strip_prefix("resources/lang/"))?;
    if let Some(locale) = rest.strip_suffix(".json") {
        return (!locale.contains('/')).then(|| (locale.to_owned(), None));
    }
    let (locale, file) = rest.split_once('/')?;
    Some((
        locale.to_owned(),
        Some(file.strip_suffix(".php")?.to_owned()),
    ))
}

pub fn translation_key(text: &str, at: usize, file: &Option<String>) -> Option<String> {
    match file {
        None => {
            let tokens = php::tokenize(text, 0, text.len());
            tokens.iter().enumerate().find_map(|(i, t)| match &t.kind {
                Kind::Str { value, .. }
                    if t.start <= at
                        && at <= t.end
                        && tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Colon) =>
                {
                    Some(value.clone())
                }
                _ => None,
            })
        }
        Some(file) => php::array_keys(&php::tokenize(text, 0, text.len()))
            .into_iter()
            .find(|k| k.start <= at && at <= k.end)
            .map(|k| format!("{file}.{}", k.path)),
    }
}

fn last_segment(key: &str) -> &str {
    key.rsplit('.').next().unwrap_or(key)
}

fn scope(text: &str) -> refs::Scope {
    crate::scope_of(text)
}

pub fn usage_edits(
    text: &str,
    blade: bool,
    kind: RefKind,
    old: &str,
    new: &str,
) -> Vec<(usize, usize, String)> {
    let sc = scope(text);
    php::args_in(text, blade)
        .into_iter()
        .filter_map(|a| {
            let seg = refs::segment(&a, a.value.len(), &sc)?;
            (seg.want == Want::Kind(kind) && a.value.get(seg.start..seg.end) == Some(old))
                .then(|| (a.start + seg.start, a.start + seg.end, new.to_owned()))
        })
        .collect()
}

pub fn lang_definition_edits(
    rel_path: &str,
    text: &str,
    old: &str,
    new: &str,
) -> Vec<(usize, usize, String)> {
    let Some((_, file)) = lang_file(rel_path) else {
        return Vec::new();
    };
    match file {
        None => {
            let tokens = php::tokenize(text, 0, text.len());
            tokens
                .iter()
                .enumerate()
                .filter_map(|(i, t)| match &t.kind {
                    Kind::Str { value, .. }
                        if value == old
                            && tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Colon) =>
                    {
                        let quoted = matches!(text.as_bytes().get(t.start), Some(b'\'' | b'"'));
                        let s = t.start + usize::from(quoted);
                        Some((s, s + value.len(), new.to_owned()))
                    }
                    _ => None,
                })
                .collect()
        }
        Some(file) => {
            let Some(path) = old.strip_prefix(&format!("{file}.")) else {
                return Vec::new();
            };
            let Some(new_path) = new.strip_prefix(&format!("{file}.")) else {
                return Vec::new();
            };
            if path.rsplit_once('.').map(|(p, _)| p) != new_path.rsplit_once('.').map(|(p, _)| p) {
                return Vec::new();
            }
            php::array_keys(&php::tokenize(text, 0, text.len()))
                .into_iter()
                .filter(|k| k.path == path)
                .map(|k| {
                    let quoted = matches!(text.as_bytes().get(k.start), Some(b'\'' | b'"'));
                    let s = k.start + usize::from(quoted);
                    (
                        s,
                        s + last_segment(path).len(),
                        last_segment(new_path).to_owned(),
                    )
                })
                .collect()
        }
    }
}

const USAGE_GLOBS: &[&str] = &[
    "app/**/*.php",
    "routes/**/*.php",
    "resources/views/**/*.php",
    "tests/**/*.php",
    "database/**/*.php",
    "config/**/*.php",
];

pub(crate) fn usage_locations(
    project: &Project,
    root: &str,
    doc: &Document,
    kind: RefKind,
    key: &str,
) -> Vec<Location> {
    locations_matching(project, root, doc, |text, blade| {
        usage_edits(text, blade, kind, key, key)
            .into_iter()
            .map(|(s, e, _)| (s, e))
            .collect()
    })
}

pub(crate) fn locations_matching(
    project: &Project,
    root: &str,
    doc: &Document,
    spans: impl Fn(&str, bool) -> Vec<(usize, usize)>,
) -> Vec<Location> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for glob in USAGE_GLOBS {
        for path in find(project, root, glob, 5000) {
            if !seen.insert(path.clone()) {
                continue;
            }
            let text = if path == doc.path {
                doc.text.clone()
            } else {
                match crate::project::read_listed(project, &path) {
                    Ok(t) => t,
                    Err(_) => continue,
                }
            };
            let lines = Lines::new(&text);
            for (s, e) in spans(&text, is_blade(&path)) {
                out.push(Location {
                    path: path.clone(),
                    range: lines.range(&text, s, e),
                });
            }
        }
    }
    out
}

fn kind_name(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Route => "route",
        RefKind::Translation => "translation",
        _ => "view",
    }
}

fn kind_of(name: &str) -> RefKind {
    match name {
        "route" => RefKind::Route,
        "translation" => RefKind::Translation,
        _ => RefKind::View,
    }
}

fn action(title: String, pending: &Pending) -> CodeAction {
    CodeAction {
        title,
        kind: Some("refactor".into()),
        preferred: false,
        edits: Vec::new(),
        data: serde_json::to_string(pending).ok(),
    }
}

impl Laravel {
    fn definition_key(
        &mut self,
        root: &str,
        doc: &Document,
        start: usize,
        local: &str,
        text: &str,
    ) -> Option<(RefKind, String)> {
        let rel_path = rel(root, &doc.path).replace('\\', "/");
        if let Some((_, file)) = lang_file(&rel_path) {
            return translation_key(text, start, &file).map(|k| (RefKind::Translation, k));
        }
        if !rel_path.starts_with("routes/") || start == 0 || !names_route(text, start - 1) {
            return None;
        }
        let dotted = format!(".{local}");
        crate::index::routes::parse(text)
            .into_iter()
            .filter_map(|r| {
                let name = r.name?;
                (r.start <= start && (name == local || name.ends_with(&dotted)))
                    .then_some((r.start, name))
            })
            .max_by_key(|(at, _)| *at)
            .map(|(_, name)| (RefKind::Route, name))
    }

    pub(crate) fn rename_actions(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Vec<CodeAction> {
        let Some(s) = string_at(&doc.text, cursor) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let disk = match self.originals.get(&path_key(&doc.path)) {
            Some(text) => text.clone(),
            None => project.read_file(&doc.path).unwrap_or_default(),
        };
        let renamed = match edited(&disk, &doc.text, s.start, s.end) {
            Some(old_local) => self
                .definition_key(root, doc, s.start, &old_local, &disk)
                .map(|(kind, old)| {
                    let base = old.strip_suffix(old_local.as_str()).unwrap_or("");
                    (kind, old.clone(), format!("{base}{}", s.value))
                }),
            None => self
                .definition_key(root, doc, s.start, &s.value, &doc.text)
                .and_then(|(kind, key)| {
                    self.saved
                        .iter()
                        .rev()
                        .find(|r| r.path == path_key(&doc.path) && r.kind == kind && r.new == key)
                        .map(|r| (kind, r.old.clone(), r.new.clone()))
                }),
        };
        if let Some((kind, old, new)) = renamed
            && old != new
            && !new.is_empty()
        {
            let pending = Pending::Rename {
                kind: kind_name(kind).into(),
                old: old.clone(),
                new: new.clone(),
                path: doc.path.clone(),
            };
            let title = match kind {
                RefKind::Translation => {
                    format!("Rename translation '{old}' to '{new}' in every locale and usage")
                }
                _ => format!("Rename route '{old}' to '{new}' in every usage"),
            };
            out.push(action(title, &pending));
        }
        out.extend(self.view_actions(project, root, doc, cursor));
        out
    }

    fn view_actions(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Vec<CodeAction> {
        let Some(arg) = php::arg_at(&doc.text, is_blade(&doc.path), cursor) else {
            return Vec::new();
        };
        let Some(seg) = refs::segment(&arg, arg.value.len(), &scope(&doc.text)) else {
            return Vec::new();
        };
        if seg.want != Want::Kind(RefKind::View) || !arg.closed || arg.concat {
            return Vec::new();
        }
        let old = arg.value[seg.start..seg.end].to_owned();
        let views = self.cache.get(RefKind::View, project, root);
        if old.is_empty() || old.contains("::") || views.iter().any(|v| v.key == old) {
            return Vec::new();
        }
        let deeper = format!(".{old}");
        let mut candidates: Vec<String> = views
            .iter()
            .filter(|v| v.key.ends_with(&deeper) || old.ends_with(&format!(".{}", v.key)))
            .map(|v| v.key.clone())
            .collect();
        if candidates.is_empty() {
            let limit = (old.len() / 3).max(2);
            let mut near: Vec<(usize, String)> = views
                .iter()
                .map(|v| (crate::checks::distance(&old, &v.key), v.key.clone()))
                .filter(|(d, _)| *d <= limit)
                .collect();
            near.sort();
            candidates = near.into_iter().map(|(_, k)| k).collect();
        }
        candidates.truncate(2);
        let lines = Lines::new(&doc.text);
        let range = lines.range(&doc.text, arg.start + seg.start, arg.start + seg.end);
        let mut out = Vec::new();
        for new in candidates {
            out.push(CodeAction {
                title: format!("Change to '{new}'"),
                kind: Some("quickfix".into()),
                preferred: out.is_empty(),
                edits: vec![(
                    doc.path.clone(),
                    vec![TextEdit {
                        range,
                        text: new.clone(),
                    }],
                )],
                data: None,
            });
            out.push(action(
                format!("Change every usage of view '{old}' to '{new}'"),
                &Pending::Views {
                    old: old.clone(),
                    new: new.clone(),
                },
            ));
        }
        out
    }

    pub(crate) fn resolve_pending(
        &mut self,
        doc: &Document,
        action: CodeAction,
    ) -> Result<CodeAction, Error> {
        let Some(pending) = action.data.as_deref().and_then(|d| {
            serde_json::from_str::<Pending>(d).or_error("reading a code action's data")
        }) else {
            return Ok(action);
        };
        let (project, root) = self
            .laravel_project(&doc.path)
            .ok_or_else(|| Error::Invalid("this file is not in a Laravel project".into()))?;
        if let Pending::FormRequest { at } = pending {
            return self.resolve_form_request(&project, doc, at, action);
        }
        let (kind, old, new, skip) = match &pending {
            Pending::Rename {
                kind,
                old,
                new,
                path,
            } => (kind_of(kind), old.clone(), new.clone(), Some(path.clone())),
            Pending::Views { old, new } => (RefKind::View, old.clone(), new.clone(), None),
            Pending::FormRequest { .. } => return Ok(action),
        };
        let mut files: Vec<(String, Vec<TextEdit>)> = Vec::new();
        let mut add = |path: String, text: &str, edits: Vec<(usize, usize, String)>| {
            if edits.is_empty() {
                return;
            }
            let lines = Lines::new(text);
            let edits = edits
                .into_iter()
                .map(|(s, e, t)| TextEdit {
                    range: lines.range(text, s, e),
                    text: t,
                })
                .collect();
            files.push((path, edits));
        };
        let mut seen = std::collections::BTreeSet::new();
        if doc.path.ends_with(".php") {
            seen.insert(doc.path.clone());
            add(
                doc.path.clone(),
                &doc.text,
                usage_edits(&doc.text, is_blade(&doc.path), kind, &old, &new),
            );
        }
        for glob in USAGE_GLOBS {
            for path in find(&project, &root, glob, 5000) {
                if !seen.insert(path.clone()) {
                    continue;
                }
                let text = if path == doc.path {
                    doc.text.clone()
                } else {
                    match crate::project::read_listed(&project, &path) {
                        Ok(t) => t,
                        Err(_) => continue,
                    }
                };
                add(
                    path.clone(),
                    &text,
                    usage_edits(&text, is_blade(&path), kind, &old, &new),
                );
            }
        }
        if kind == RefKind::Translation {
            for glob in [
                "lang/**/*.php",
                "lang/*.json",
                "resources/lang/**/*.php",
                "resources/lang/*.json",
            ] {
                for path in find(&project, &root, glob, 2000) {
                    if skip.as_deref().map(path_key) == Some(path_key(&path))
                        || !seen.insert(path.clone())
                    {
                        continue;
                    }
                    let Ok(text) = crate::project::read_listed(&project, &path) else {
                        continue;
                    };
                    let rel_path = rel(&root, &path).replace('\\', "/");
                    add(
                        path.clone(),
                        &text,
                        lang_definition_edits(&rel_path, &text, &old, &new),
                    );
                }
            }
        }
        if files.is_empty() {
            return Err(Error::NotFound(format!("no usage of '{old}' to change")));
        }
        self.saved.retain(|r| r.old != old);
        Ok(CodeAction {
            edits: files,
            data: None,
            ..action
        })
    }

    pub(crate) fn form_request_action(
        &mut self,
        project: &Project,
        doc: &Document,
        cursor: usize,
    ) -> Vec<CodeAction> {
        if !doc.path.ends_with(".php") || is_blade(&doc.path) || !doc.text.contains("validate(") {
            return Vec::new();
        }
        let taken = |rel_path: &str| project.read_file(rel_path).is_ok();
        match crate::formrequest::extract(&doc.text, cursor, None, &taken) {
            Some(x) => vec![action(
                format!("Extract the validation into a Form Request ({})", x.class),
                &Pending::FormRequest { at: cursor },
            )],
            None => Vec::new(),
        }
    }

    pub(crate) fn method_routes(
        &mut self,
        project: &Project,
        root: &str,
        path: &str,
        text: &str,
        cursor: usize,
    ) -> Vec<RouteTarget> {
        if !path.replace('\\', "/").contains("/Http/Controllers/") {
            return Vec::new();
        }
        let Some(class) = crate::index::parse_class(text) else {
            return Vec::new();
        };
        let Some(method) = class.methods.iter().find(|m| {
            let line_start = text[..m.start].rfind('\n').map_or(0, |i| i + 1);
            m.public && line_start <= cursor && cursor <= m.body.0
        }) else {
            return Vec::new();
        };
        let wanted = |key: &str| {
            key.split_once('@').is_some_and(|(c, f)| {
                f == method.name && c.rsplit('\\').next() == Some(class.name.as_str())
            })
        };
        let mut seen = std::collections::BTreeSet::new();
        self.cache
            .get(RefKind::RouteAction, project, root)
            .iter()
            .filter(|e| wanted(&e.key))
            .filter_map(|e| {
                let detail = e.detail.clone().unwrap_or_default();
                let (head, name) = detail.split_once(" · ").unwrap_or((detail.as_str(), ""));
                let (verb, uri) = head.split_once(' ')?;
                let (verb, uri) = if verb == "resource" {
                    crate::routetools::resource_route(uri, &method.name)?
                } else {
                    (
                        verb.split('|').next().unwrap_or(verb).to_owned(),
                        uri.to_owned(),
                    )
                };
                let action = e.key.replace('@', "::");
                let file = crate::routetools::test_path(&action, &verb, &uri);
                seen.insert(file.clone()).then(|| RouteTarget {
                    exists: project.read_file(&file).is_ok(),
                    method: verb,
                    uri,
                    name: name.to_owned(),
                    action,
                    file,
                })
            })
            .collect()
    }

    pub(crate) fn generate_test_command(
        &mut self,
        ctx: &Invocation,
    ) -> Result<Option<String>, Error> {
        let editor = ctx
            .editor()
            .ok_or_else(|| Error::Invalid("Open the controller in the editor first.".into()))?;
        let path = editor.path();
        let (project, root) = self
            .laravel_project(&path)
            .ok_or("this file is not in a Laravel project")?;
        let doc = editor.document()?;
        let sel = editor.selections()?.into_iter().next().ok_or_else(|| {
            Error::Invalid("Place the cursor on a controller method first.".into())
        })?;
        let cursor = offset(&doc.text, sel.start).unwrap_or(0);
        let mut routes = self.method_routes(&project, &root, &path, &doc.text, cursor);
        let target = match routes.len() {
            0 => {
                return Err(Error::Invalid(
                    "Place the cursor on the signature of a controller method that a route calls."
                        .into(),
                ));
            }
            1 => routes.remove(0),
            _ => {
                let items: Vec<openidea_plugin::PickItem> = routes
                    .iter()
                    .map(|r| openidea_plugin::PickItem::new(&r.file, &r.title()))
                    .collect();
                let Some(chosen) = host::pick("Feature test for which route?", &items)? else {
                    return Ok(None);
                };
                let at = routes.iter().position(|r| r.file == chosen).unwrap_or(0);
                routes.remove(at)
            }
        };
        if !target.exists {
            self.write_route_test(
                &project,
                &root,
                &target.method,
                &target.uri,
                &target.name,
                &target.action,
            )?;
        }
        host::open_location(&crate::project::join(&root, &target.file), 0, 0)?;
        Ok(Some(if target.exists {
            format!("Opened {}", target.file)
        } else {
            format!("Created {}", target.file)
        }))
    }

    fn resolve_form_request(
        &mut self,
        project: &Project,
        doc: &Document,
        at: usize,
        action: CodeAction,
    ) -> Result<CodeAction, Error> {
        let taken = |rel_path: &str| project.read_file(rel_path).is_ok();
        let x = crate::formrequest::extract(&doc.text, at, None, &taken)
            .ok_or_else(|| Error::Invalid("the validate() call isn't there anymore".into()))?;
        create_form_request(project, &x)?;
        Ok(CodeAction {
            edits: vec![(doc.path.clone(), text_edits(&doc.text, x.edits))],
            data: None,
            ..action
        })
    }

    pub(crate) fn extract_form_request(
        &mut self,
        ctx: &Invocation,
    ) -> Result<Option<String>, Error> {
        let editor = ctx
            .editor()
            .ok_or_else(|| Error::Invalid("Open the controller in the editor first.".into()))?;
        let path = editor.path();
        let (project, _) = self
            .laravel_project(&path)
            .ok_or("this file is not in a Laravel project")?;
        let doc = editor.document()?;
        let sel = editor.selections()?.into_iter().next().ok_or_else(|| {
            Error::Invalid("Place the cursor in the validate() call first.".into())
        })?;
        let start = offset(&doc.text, sel.start).unwrap_or(0);
        let end = offset(&doc.text, sel.end).unwrap_or(start).max(start);
        let mut candidates = vec![start];
        if let Some(i) = doc.text[start..end].find("validate(") {
            candidates.push(start + i + "validate(".len());
        }
        if let Some(i) = doc.text[end..].find("validate(").filter(|i| *i < 80) {
            candidates.push(end + i + "validate(".len());
        }
        candidates.push(end);
        let taken = |rel_path: &str| project.read_file(rel_path).is_ok();
        let (at, suggested) = candidates
            .into_iter()
            .find_map(|at| {
                crate::formrequest::extract(&doc.text, at, None, &taken).map(|x| (at, x))
            })
            .ok_or_else(|| {
                Error::Invalid(
                    "Place the cursor in or select a $request->validate([...]) call to extract it into a Form Request."
                        .into(),
                )
            })?;
        let mut prompt = "Form Request class (in app/Http/Requests)".to_owned();
        let mut value = suggested.class;
        let name = loop {
            let Some(typed) = host::input(&prompt, Some(&value))? else {
                return Ok(None);
            };
            match crate::formrequest::class_name(&typed) {
                Ok(name) if taken(&crate::formrequest::file_of(&name)) => {
                    prompt = format!(
                        "{} already exists: choose another name",
                        crate::formrequest::file_of(&name)
                    );
                    value = typed;
                }
                Ok(name) => break name,
                Err(why) => {
                    prompt = why;
                    value = typed;
                }
            }
        };
        let current = editor.document()?;
        if current.version != doc.version {
            return Err(Error::Invalid(
                "The file changed while the dialog was open: run Extract Form Request again."
                    .into(),
            ));
        }
        let x = crate::formrequest::extract(&doc.text, at, Some(&name), &taken)
            .ok_or_else(|| Error::Invalid("the validate() call isn't there anymore".into()))?;
        create_form_request(&project, &x)?;
        editor.apply_edits(doc.version, &text_edits(&doc.text, x.edits))?;
        Ok(Some(format!("Created {}", x.file)))
    }

    pub(crate) fn remember_original(&mut self, root: &str, path: &str, text: &str, replace: bool) {
        let rel_path = rel(root, path).replace('\\', "/");
        if defined_keys(&rel_path, "").is_none() {
            return;
        }
        let key = path_key(path);
        match self.originals.get(&key) {
            Some(old) if !replace || old == text => return,
            Some(old) => {
                if let (Some((kind, before)), Some((_, after))) =
                    (defined_keys(&rel_path, old), defined_keys(&rel_path, text))
                {
                    self.remember_renames(&key, kind, before, after);
                }
            }
            None => {}
        }
        self.originals.insert(key.clone(), text.to_owned());
        if self.originals.len() > 64 {
            let first = self.originals.keys().next().cloned();
            if let Some(k) = first.filter(|k| *k != key) {
                self.originals.remove(&k);
            }
        }
    }

    fn remember_renames(&mut self, path: &str, kind: RefKind, old: Vec<String>, new: Vec<String>) {
        if old.len() != new.len() {
            return;
        }
        for (o, n) in old.into_iter().zip(new) {
            if o == n {
                continue;
            }
            let old = self
                .saved
                .iter()
                .position(|r| r.path == path && r.kind == kind && r.new == o)
                .map_or(o, |i| self.saved.remove(i).old);
            if old != n {
                self.saved.push(Saved {
                    path: path.to_owned(),
                    kind,
                    old,
                    new: n,
                });
            }
        }
        if self.saved.len() > 32 {
            let extra = self.saved.len() - 32;
            self.saved.drain(..extra);
        }
    }
}

pub(crate) fn path_key(path: &str) -> String {
    let mut key = path.replace('\\', "/");
    if key.as_bytes().get(1) == Some(&b':') {
        key[..1].make_ascii_lowercase();
    }
    key
}

fn defined_keys(rel_path: &str, text: &str) -> Option<(RefKind, Vec<String>)> {
    if rel_path.starts_with("routes/") {
        let names = crate::index::routes::parse(text)
            .into_iter()
            .filter_map(|r| r.name)
            .collect();
        return Some((RefKind::Route, names));
    }
    let (_, file) = lang_file(rel_path)?;
    let tokens = php::tokenize(text, 0, text.len());
    let keys = match file {
        None => tokens
            .iter()
            .enumerate()
            .filter_map(|(i, t)| match &t.kind {
                Kind::Str { value, .. }
                    if tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Colon) =>
                {
                    Some(value.clone())
                }
                _ => None,
            })
            .collect(),
        Some(file) => php::array_keys(&tokens)
            .into_iter()
            .map(|k| format!("{file}.{}", k.path))
            .collect(),
    };
    Some((RefKind::Translation, keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_what_was_edited_in_a_string() {
        let disk = "Route::get('/', X::class)->name('orders.index');\n";
        let buffer = "Route::get('/', X::class)->name('orders.list');\n";
        let at = buffer.find("orders.list").unwrap();
        assert_eq!(
            edited(disk, buffer, at, at + "orders.list".len()).as_deref(),
            Some("orders.index")
        );
        assert_eq!(edited(disk, disk, at, at + "orders.index".len()), None);
        let elsewhere = "Route::get('/x', X::class)->name('orders.list');\n";
        assert_eq!(
            edited(
                disk,
                elsewhere,
                elsewhere.find("orders").unwrap(),
                elsewhere.find("orders").unwrap() + 11
            ),
            None
        );
    }

    #[test]
    fn recognises_route_names_and_lang_files() {
        let text =
            "Route::get('/', X::class)\n    ->name('home');\nRoute::view('/a', 'welcome');\n";
        assert!(names_route(text, text.find("'home'").unwrap()));
        assert!(!names_route(text, text.find("'welcome'").unwrap()));
        assert_eq!(
            lang_file("lang/pt_BR/auth.php"),
            Some(("pt_BR".into(), Some("auth".into())))
        );
        assert_eq!(lang_file("lang/es.json"), Some(("es".into(), None)));
        assert_eq!(
            lang_file("resources/lang/en/admin/users.php"),
            Some(("en".into(), Some("admin/users".into())))
        );
        assert_eq!(lang_file("app/Lang.php"), None);
    }

    #[test]
    fn translation_keys_and_their_definitions_in_other_locales() {
        let text = "<?php\nreturn [\n    'failed' => 'Wrong.',\n    'password' => [\n        'reset' => 'Reset.',\n    ],\n];\n";
        let file = Some("auth".to_owned());
        assert_eq!(
            translation_key(text, text.find("reset").unwrap(), &file).as_deref(),
            Some("auth.password.reset")
        );
        let edits = lang_definition_edits(
            "lang/pt/auth.php",
            text,
            "auth.password.reset",
            "auth.password.renew",
        );
        assert_eq!(edits.len(), 1);
        assert_eq!(&text[edits[0].0..edits[0].1], "reset");
        assert_eq!(edits[0].2, "renew");
        let json = "{\n    \"Welcome\": \"Bem-vindo\"\n}\n";
        let edits = lang_definition_edits("lang/pt.json", json, "Welcome", "Hello");
        assert_eq!(&json[edits[0].0..edits[0].1], "Welcome");
    }

    #[test]
    fn paths_compare_with_either_slash_and_drive_case() {
        assert_eq!(
            path_key(r"C:\Users\p\routes\web.php"),
            path_key("c:/Users/p/routes/web.php")
        );
        assert_eq!(
            path_key("/project/routes/web.php"),
            "/project/routes/web.php"
        );
    }

    #[test]
    fn route_names_and_translation_keys_defined_by_a_file() {
        let routes = "<?php\nRoute::get('/', X::class)->name('home');\nRoute::get('/a', Y::class)->name('a.index');\n";
        assert_eq!(
            defined_keys("routes/web.php", routes),
            Some((RefKind::Route, vec!["home".into(), "a.index".into()]))
        );
        let lang = "<?php\nreturn ['title' => 'T', 'nav' => ['home' => 'H']];\n";
        let (kind, keys) = defined_keys("lang/en/shop.php", lang).unwrap();
        assert_eq!(kind, RefKind::Translation);
        assert!(
            keys.contains(&"shop.title".to_owned()) && keys.contains(&"shop.nav.home".to_owned()),
            "{keys:?}"
        );
        assert_eq!(defined_keys("app/Models/User.php", ""), None);
    }

    #[test]
    fn pending_actions_round_trip_as_json() {
        let p = Pending::Rename {
            kind: "route".into(),
            old: "a".into(),
            new: "b".into(),
            path: "/p/routes/web.php".into(),
        };
        let text = serde_json::to_string(&p).unwrap();
        assert!(text.contains("\"op\":\"rename\""), "{text}");
        assert_eq!(serde_json::from_str::<Pending>(&text).unwrap(), p);
    }
}

#[derive(Serialize, Deserialize)]
struct RouteTestRef {
    method: String,
    uri: String,
    name: String,
    action: String,
}

impl FileActions for Laravel {
    fn file_actions(&mut self, doc: &Document, selection: Range) -> Vec<FileAction> {
        let Some((project, root)) = self.laravel_project(&doc.path) else {
            return Vec::new();
        };
        let Some(cursor) = offset(&doc.text, selection.start) else {
            return Vec::new();
        };
        self.method_routes(&project, &root, &doc.path, &doc.text, cursor)
            .into_iter()
            .map(|r| FileAction {
                title: r.title(),
                kind: Some("refactor".into()),
                data: serde_json::to_string(&RouteTestRef {
                    method: r.method,
                    uri: r.uri,
                    name: r.name,
                    action: r.action,
                })
                .ok(),
                ..FileAction::default()
            })
            .collect()
    }

    fn resolve_file_action(
        &mut self,
        doc: &Document,
        action: FileAction,
    ) -> Result<FileAction, Error> {
        let Some(r) = action
            .data
            .as_deref()
            .and_then(|d| serde_json::from_str::<RouteTestRef>(d).ok())
        else {
            return Ok(action);
        };
        let (project, root) = self
            .laravel_project(&doc.path)
            .ok_or_else(|| Error::Invalid("this file is not in a Laravel project".into()))?;
        let file = crate::routetools::test_path(&r.action, &r.method, &r.uri);
        let operations = if project.read_file(&file).is_ok() {
            Vec::new()
        } else {
            let test = self.route_test(&project, &root, &r.method, &r.uri, &r.name, &r.action);
            vec![FileOperation::Create {
                path: test.file,
                text: test.content,
                overwrite: false,
            }]
        };
        Ok(FileAction {
            operations,
            open: Some(file),
            data: None,
            ..action
        })
    }
}
