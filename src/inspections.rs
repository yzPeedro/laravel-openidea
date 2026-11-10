use crate::Laravel;
use crate::audit::Logged;
use crate::checks::{self, Columns, Enabled, Facts, Finding, ModelFacts, Parts};
use crate::index::RefKind;
use crate::project::{Lines, project_of};
use openidea_plugin::providers::{CodeAction, Document};
use openidea_plugin::{Diagnostic, Project, Severity, TextEdit, host};
use std::collections::BTreeMap;

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn partner(path: &str) -> Option<(String, bool)> {
    let name = file_name(path);
    let dir = &path[..path.len() - name.len()];
    match name {
        ".env" => Some((format!("{dir}.env.example"), false)),
        ".env.example" => Some((format!("{dir}.env"), true)),
        _ => None,
    }
}

fn env_key(value: &str) -> Option<String> {
    let (at, _) = value
        .match_indices("env(")
        .find(|(i, _)| !value[..*i].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))?;
    let rest = value[at + 4..].trim_start();
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let key = rest[1..].split(quote).next()?;
    (!key.is_empty()).then(|| key.to_owned())
}

fn setting(name: &str) -> bool {
    host::setting_bool(name).unwrap_or(true)
}

impl Laravel {
    pub(crate) fn cached_facts(&mut self, project: &Project, root: &str) -> std::rc::Rc<Parts> {
        if let Some(facts) = self.facts_cache.get(root) {
            return facts.clone();
        }
        let facts = std::rc::Rc::new(self.check_facts(project, root));
        self.facts_cache.insert(root.to_owned(), facts.clone());
        facts
    }

    pub(crate) fn facts_ready(&self, root: &str) -> bool {
        self.facts_cache.contains_key(root)
    }

    fn check_facts(&mut self, project: &Project, root: &str) -> Parts {
        let relations: Vec<(String, String)> = self
            .cache
            .get(RefKind::Relation, project, root)
            .iter()
            .filter_map(|e| {
                e.key
                    .split_once('.')
                    .map(|(m, r)| (m.to_owned(), r.to_owned()))
            })
            .collect();
        let psr4 = crate::casts::psr4(&project.read_file("composer.json").unwrap_or_default());
        let models = self
            .cache
            .get(RefKind::Model, project, root)
            .iter()
            .filter_map(|e| {
                let table = e.detail.as_deref()?.strip_prefix("table ")?.to_owned();
                let source =
                    crate::project::read_listed(project, &e.location.path).unwrap_or_default();
                Some(ModelFacts {
                    name: e.key.clone(),
                    table,
                    path: e.location.path.clone(),
                    enums: crate::enums::of_model(project, &psr4, &source),
                    source,
                    relations: relations
                        .iter()
                        .filter(|(m, _)| *m == e.key)
                        .map(|(_, r)| r.clone())
                        .collect(),
                })
            })
            .collect();
        let mut columns: Columns = BTreeMap::new();
        for e in self.cache.get(RefKind::Column, project, root).iter() {
            if let Some((t, c)) = e.key.split_once('.') {
                columns
                    .entry(t.to_owned())
                    .or_default()
                    .insert(c.to_owned());
            }
        }
        let mut sources: BTreeMap<String, String> = BTreeMap::new();
        let config_env = self
            .cache
            .get(RefKind::Config, project, root)
            .iter()
            .filter_map(|e| {
                let source = sources.entry(e.location.path.clone()).or_insert_with(|| {
                    crate::project::read_listed(project, &e.location.path).unwrap_or_default()
                });
                let line = source.lines().nth(e.location.range.start.line as usize)?;
                let value = line.split_once("=>").map_or(line, |(_, v)| v);
                env_key(value).map(|k| (e.key.clone(), k))
            })
            .collect();
        (models, columns, config_env)
    }

    pub(crate) fn findings(
        &mut self,
        project: &Project,
        root: &str,
        path: &str,
        text: &str,
    ) -> Vec<Finding> {
        if let Some((other, example)) = partner(path) {
            if !setting("envKeysInspection") {
                return Vec::new();
            }
            return match project.read_file(&other) {
                Ok(other_text) => checks::dotenv(path, text, &other, &other_text, example),
                Err(_) => Vec::new(),
            };
        }
        if path.ends_with(".blade.php") {
            return if setting("securityInspection") {
                crate::security::blade(text, path)
            } else {
                Vec::new()
            };
        }
        if !path.ends_with(".php") {
            return Vec::new();
        }
        let enabled = Enabled {
            columns: setting("unknownColumnInspection"),
            env: setting("envCallInspection"),
            fillable: setting("fillableInspection"),
            n_plus_one: setting("nPlusOneInspection"),
            duplicate_keys: setting("duplicateKeyInspection"),
            enum_values: setting("enumValueInspection"),
            security: setting("securityInspection"),
        };
        if !(enabled.columns
            || enabled.env
            || enabled.fillable
            || enabled.n_plus_one
            || enabled.duplicate_keys
            || enabled.enum_values
            || enabled.security
            || setting("upgradeInspection"))
        {
            return Vec::new();
        }
        let facts = self.cached_facts(project, root);
        let (models, columns, config_env) = &*facts;
        let rel = crate::project::rel(root, path).replace('\\', "/");
        let mut out = checks::php(
            text,
            &Facts {
                path,
                rel_path: &rel,
                models,
                columns,
                config_env,
                enabled,
            },
        );
        if setting("upgradeInspection")
            && let Some(installed) = self.installed_major(project, root)
        {
            let composer = if installed == 12 {
                project.read_file("composer.json").unwrap_or_default()
            } else {
                String::new()
            };
            out.extend(crate::upgrade::findings(
                text, path, &rel, &composer, installed,
            ));
            out.sort_by_key(|f| (f.start, f.end));
        }
        out
    }

    pub(crate) fn installed_major(&mut self, project: &Project, root: &str) -> Option<u32> {
        let status = self
            .statuses
            .entry(root.to_owned())
            .or_insert_with(|| {
                crate::project::laravel_version(project)
                    .map_or("Laravel".to_owned(), |v| format!("Laravel {v}"))
            })
            .clone();
        crate::upgrade::major(status.strip_prefix("Laravel ")?)
    }

    pub(crate) fn finding_diagnostics(text: &str, findings: &[Finding]) -> Vec<Diagnostic> {
        let lines = Lines::new(text);
        findings
            .iter()
            .map(|f| {
                Diagnostic::new(
                    lines.range(text, f.start, f.end),
                    if f.info {
                        Severity::Info
                    } else {
                        Severity::Warning
                    },
                    f.message.clone(),
                )
                .code(f.code)
            })
            .collect()
    }

    pub(crate) fn publish_env(&mut self, path: &str) -> bool {
        let Some((other, _)) = partner(path) else {
            return false;
        };
        let Some((project, root)) = self.laravel_project(path) else {
            return true;
        };
        for p in [path.to_owned(), other] {
            let (items, exists) = match project.read_file(&p) {
                Ok(text) => {
                    let found = self.findings(&project, &root, &p, &text);
                    (Self::finding_diagnostics(&text, &found), true)
                }
                Err(_) => (Vec::new(), false),
            };
            let published = host::publish_diagnostics(&p, &items);
            if exists {
                published.or_error(&format!("publishing diagnostics for {p}"));
            }
        }
        true
    }

    pub(crate) fn fix_actions(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Vec<CodeAction> {
        let findings = self.findings(project, root, &doc.path, &doc.text);
        let mut out = Vec::new();
        for f in findings
            .into_iter()
            .filter(|f| f.start <= cursor && cursor <= f.end)
        {
            for fix in f.fixes {
                let mut files: Vec<(String, Vec<TextEdit>)> = Vec::new();
                for edit in fix.edits {
                    let text = if edit.path == doc.path {
                        doc.text.clone()
                    } else {
                        project_of(&edit.path)
                            .and_then(|p| p.read_file(&edit.path).ok())
                            .unwrap_or_default()
                    };
                    let range = Lines::new(&text).range(
                        &text,
                        edit.start.min(text.len()),
                        edit.end.min(text.len()),
                    );
                    let item = TextEdit {
                        range,
                        text: edit.text,
                    };
                    match files.iter_mut().find(|(p, _)| *p == edit.path) {
                        Some((_, edits)) => edits.push(item),
                        None => files.push((edit.path, vec![item])),
                    }
                }
                out.push(CodeAction {
                    title: fix.title,
                    kind: Some("quickfix".into()),
                    preferred: out.is_empty(),
                    edits: files,
                    data: None,
                });
            }
        }
        out
    }
}
