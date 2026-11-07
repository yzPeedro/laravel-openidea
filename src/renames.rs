use crate::Laravel;
use crate::audit::Logged;
use crate::index::RefKind;
use crate::php::{self, Kind, is_blade};
use crate::project::{Lines, find, rel};
use crate::refs::{self, Want};
use openidea_plugin::providers::{CodeAction, Document};
use openidea_plugin::{Error, Location, Project, TextEdit};
use serde::{Deserialize, Serialize};

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
            for (s, e, _) in usage_edits(&text, is_blade(&path), kind, key, key) {
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
        let disk = match self.originals.get(&doc.path) {
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
                        .find(|r| r.path == doc.path && r.kind == kind && r.new == key)
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
                    if skip.as_deref() == Some(path.as_str()) || !seen.insert(path.clone()) {
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
        match crate::formrequest::extract(&doc.text, cursor, &taken) {
            Some(x) => vec![action(
                format!("Extract the validation into a Form Request ({})", x.class),
                &Pending::FormRequest { at: cursor },
            )],
            None => Vec::new(),
        }
    }

    fn resolve_form_request(
        &mut self,
        project: &Project,
        doc: &Document,
        at: usize,
        action: CodeAction,
    ) -> Result<CodeAction, Error> {
        let taken = |rel_path: &str| project.read_file(rel_path).is_ok();
        let x = crate::formrequest::extract(&doc.text, at, &taken)
            .ok_or_else(|| Error::Invalid("the validate() call isn't there anymore".into()))?;
        let mode = crate::runner::detect(project);
        let args = mode.php_args(&[
            "-r".into(),
            crate::devtools::script(crate::devtools::CREATE_FILE_PHP),
            "--".into(),
            x.file.clone(),
        ]);
        let out = match project.run_process(mode.program(), &args, Some(&x.content)) {
            Ok(out) => out,
            Err(Error::Denied(cap)) => {
                return Err(Error::Failed(format!(
                    "Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to create the Form Request."
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
            return Err(Error::Failed(format!("Couldn't create {}: {why}", x.file)));
        }
        let lines = Lines::new(&doc.text);
        let edits = x
            .edits
            .into_iter()
            .map(|(s, e, t)| TextEdit {
                range: lines.range(&doc.text, s, e),
                text: t,
            })
            .collect();
        Ok(CodeAction {
            edits: vec![(doc.path.clone(), edits)],
            data: None,
            ..action
        })
    }

    pub(crate) fn remember_original(&mut self, root: &str, path: &str, text: &str, replace: bool) {
        let rel_path = rel(root, path).replace('\\', "/");
        if !(rel_path.starts_with("routes/") || lang_file(&rel_path).is_some()) {
            return;
        }
        if replace || !self.originals.contains_key(path) {
            self.originals.insert(path.to_owned(), text.to_owned());
        }
        if self.originals.len() > 64 {
            let first = self.originals.keys().next().cloned();
            if let Some(k) = first.filter(|k| k != path) {
                self.originals.remove(&k);
            }
        }
    }

    pub(crate) fn snapshot_keys(
        &mut self,
        project: &Project,
        root: &str,
        path: &str,
    ) -> Vec<(RefKind, Vec<String>)> {
        let rel_path = rel(root, path).replace('\\', "/");
        let kind = if rel_path.starts_with("routes/") {
            RefKind::Route
        } else if lang_file(&rel_path).is_some() {
            RefKind::Translation
        } else {
            return Vec::new();
        };
        let keys = self
            .cache
            .get(kind, project, root)
            .iter()
            .filter(|e| e.location.path == path)
            .map(|e| (e.location.range.start, e.key.clone()))
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .collect();
        vec![(kind, keys)]
    }

    pub(crate) fn remember_renames(
        &mut self,
        path: &str,
        before: Vec<(RefKind, Vec<String>)>,
        after: Vec<(RefKind, Vec<String>)>,
    ) {
        for ((kind, old), (_, new)) in before.into_iter().zip(after) {
            if old.len() != new.len() {
                continue;
            }
            for (o, n) in old.into_iter().zip(new) {
                if o != n {
                    self.saved.retain(|r| !(r.path == path && r.new == o));
                    self.saved.push(Saved {
                        path: path.to_owned(),
                        kind,
                        old: o,
                        new: n,
                    });
                }
            }
        }
        if self.saved.len() > 32 {
            let extra = self.saved.len() - 32;
            self.saved.drain(..extra);
        }
    }
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
