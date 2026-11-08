use crate::Laravel;
use crate::configfiles::{self, XmlWant};
use crate::index::{Entry, RefKind};
use crate::pest;
use crate::project::{Lines, find};
use crate::toolconfig::{self, Completion};
use openidea_plugin::Project;
use openidea_plugin::providers::{CompletionItem, CompletionKind, Document};

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn items(doc: &Document, c: Completion) -> Vec<CompletionItem> {
    let range = Lines::new(&doc.text).range(&doc.text, c.start, c.end);
    c.items
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let mut item = CompletionItem::new(
                &s.label,
                if s.value {
                    CompletionKind::Value
                } else {
                    CompletionKind::Property
                },
            );
            item.detail = Some(s.detail);
            item.replace = Some(range);
            item.sort_text = Some(format!("{i:04}"));
            item
        })
        .collect()
}

impl Laravel {
    pub(crate) fn tool_completion(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<Vec<CompletionItem>> {
        let name = file_name(&doc.path);
        if let Some(found) = self.config_file_completion(project, root, doc, cursor, name) {
            return Some(found);
        }
        let found = match name {
            "pint.json" => toolconfig::pint(&doc.text, cursor),
            ".php-cs-fixer.php" | ".php-cs-fixer.dist.php" => {
                toolconfig::cs_fixer(&doc.text, cursor)
            }
            n if n.ends_with(".neon") || n.ends_with(".neon.dist") => {
                toolconfig::neon(&doc.text, cursor)
            }
            _ => None,
        };
        if let Some(c) = found {
            return Some(items(doc, c));
        }
        let unix = doc.path.replace('\\', "/");
        if !unix.ends_with(".php") || !(unix.contains("/tests/") || name == "Pest.php") {
            return None;
        }
        let (start, end, want) = pest::want_at(&doc.text, cursor)?;
        let names = self.pest.entry(root.to_owned()).or_insert_with(|| {
            let mut names = pest::Names::default();
            for path in find(project, root, "tests/**/*.php", 3000) {
                if let Ok(text) = crate::project::read_listed(project, &path) {
                    pest::collect(&text, &mut names);
                }
            }
            names
        });
        let mut current = names.clone();
        pest::collect(&doc.text, &mut current);
        let (list, detail) = match want {
            pest::Want::Dataset => (current.datasets, "Pest dataset"),
            pest::Want::Group => (current.groups, "test group"),
        };
        let c = Completion {
            start,
            end,
            items: list
                .into_iter()
                .map(|l| toolconfig::Suggestion {
                    label: l,
                    detail: detail.to_owned(),
                    value: true,
                })
                .collect(),
        };
        Some(items(doc, c))
    }

    fn config_file_completion(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
        name: &str,
    ) -> Option<Vec<CompletionItem>> {
        let text = &doc.text;
        let lower = name.to_ascii_lowercase();
        if lower == "composer.json" {
            let lock = project.read_file("composer.lock").unwrap_or_default();
            let c = configfiles::composer(text, cursor, &configfiles::packages(&lock))?;
            return Some(items(doc, c));
        }
        if matches!(
            lower.as_str(),
            "phpunit.xml" | "phpunit.xml.dist" | "phpunit.dist.xml"
        ) {
            let xsd = project
                .read_file("vendor/phpunit/phpunit/phpunit.xsd")
                .unwrap_or_default();
            let schema = configfiles::phpunit_schema(&xsd);
            let (start, end, want) = configfiles::xml_context(text, cursor)?;
            let suggestions = match &want {
                XmlWant::Value { tag, attr, .. }
                    if attr == "name" && matches!(tag.as_str(), "env" | "server") =>
                {
                    let mut seen = std::collections::BTreeSet::new();
                    self.env_key_entries(project, root)
                        .into_iter()
                        .filter(|e| seen.insert(e.key.clone()))
                        .map(|e| suggestion(&e))
                        .collect()
                }
                XmlWant::Value { tag, attr, name }
                    if attr == "value" && matches!(tag.as_str(), "env" | "server") =>
                {
                    let (_, entries) = self.env_value_entries(project, root, name.as_deref()?)?;
                    entries.iter().map(suggestion).collect()
                }
                XmlWant::Text { tag } if matches!(tag.as_str(), "directory" | "file") => {
                    folders(project, root, tag == "file")
                }
                _ => configfiles::phpunit_static(&want, &schema)?,
            };
            return Some(items(
                doc,
                Completion {
                    start,
                    end,
                    items: suggestions,
                },
            ));
        }
        let ext = lower.rsplit('.').next().unwrap_or_default();
        if !matches!(
            ext,
            "js" | "ts" | "mjs" | "mts" | "cjs" | "jsx" | "tsx" | "vue" | "svelte"
        ) {
            return None;
        }
        if lower.starts_with("vite.config.") {
            let (start, end, _) = configfiles::quoted_at(text, cursor)?;
            let list: Vec<toolconfig::Suggestion> = find(project, root, "resources/**/*", 3000)
                .into_iter()
                .filter_map(|p| {
                    let rel = crate::project::rel(root, &p).replace(std::path::MAIN_SEPARATOR, "/");
                    let ext = rel.rsplit('.').next()?.to_owned();
                    [
                        "css", "scss", "sass", "less", "js", "ts", "jsx", "tsx", "vue", "svelte",
                    ]
                    .contains(&ext.as_str())
                    .then(|| toolconfig::value(&rel, "Vite entry"))
                })
                .collect();
            return Some(items(
                doc,
                Completion {
                    start,
                    end,
                    items: list,
                },
            ));
        }
        let (start, end, call) = configfiles::js_call(text, cursor)?;
        let kind = match call.as_str() {
            "route" => RefKind::Route,
            "__" | "trans" | "trans_choice" | "$t" | "t" | "$tc" | "wTrans" | "transChoice" => {
                RefKind::Translation
            }
            _ => return None,
        };
        let mut seen = std::collections::BTreeSet::new();
        let list = self
            .cache
            .get(kind, project, root)
            .iter()
            .filter(|e| seen.insert(e.key.clone()))
            .map(suggestion)
            .collect();
        Some(items(
            doc,
            Completion {
                start,
                end,
                items: list,
            },
        ))
    }
}

fn suggestion(e: &Entry) -> toolconfig::Suggestion {
    toolconfig::value(&e.key, e.detail.as_deref().unwrap_or_default())
}

fn folders(project: &Project, root: &str, files: bool) -> Vec<toolconfig::Suggestion> {
    let mut out = std::collections::BTreeSet::new();
    for base in ["tests", "app", "src"] {
        for path in find(project, root, &format!("{base}/**/*.php"), 3000) {
            let rel = crate::project::rel(root, &path).replace(std::path::MAIN_SEPARATOR, "/");
            if files {
                out.insert(rel.clone());
            }
            let mut dir = rel.as_str();
            while let Some(i) = dir.rfind('/') {
                dir = &dir[..i];
                out.insert(dir.to_owned());
            }
        }
    }
    out.into_iter()
        .map(|p| {
            let detail = if p.ends_with(".php") {
                "file"
            } else {
                "folder"
            };
            toolconfig::value(&p, detail)
        })
        .collect()
}
