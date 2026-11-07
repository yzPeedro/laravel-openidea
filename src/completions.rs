use crate::Laravel;
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
}
