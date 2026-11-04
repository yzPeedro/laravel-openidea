use super::Entry;
use crate::project::{find, rel};
use openidea_plugin::{Project, Range};
use std::collections::BTreeSet;

const PAGE_EXTENSIONS: [&str; 5] = ["vue", "tsx", "jsx", "svelte", "ts"];

pub fn inertia_pages(project: &Project, root: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for base in [
        "resources/js/Pages/",
        "resources/js/pages/",
        "resources/ts/Pages/",
    ] {
        for ext in PAGE_EXTENSIONS {
            for path in find(project, root, &format!("{base}**/*.{ext}"), 5000) {
                let relative = rel(root, &path).replace('\\', "/");
                let Some(name) = relative
                    .strip_prefix(base)
                    .and_then(|n| n.strip_suffix(&format!(".{ext}")))
                else {
                    continue;
                };
                if seen.insert(name.to_owned()) {
                    out.push(Entry::new(
                        name,
                        Some(relative.clone()),
                        &path,
                        Range::default(),
                    ));
                }
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn assets(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for dir in ["css", "js", "ts", "sass", "scss", "images", "img", "fonts"] {
        for path in find(project, root, &format!("resources/{dir}/**/*"), 3000) {
            let relative = rel(root, &path).replace('\\', "/");
            out.push(Entry::new(
                relative,
                Some("vite".into()),
                &path,
                Range::default(),
            ));
        }
    }
    for path in find(project, root, "public/**/*", 3000) {
        let relative = rel(root, &path).replace('\\', "/");
        let Some(public) = relative.strip_prefix("public/") else {
            continue;
        };
        if public.ends_with(".php")
            || public.starts_with('.')
            || public.starts_with("storage/")
            || public.starts_with("build/")
        {
            continue;
        }
        out.push(Entry::new(
            public,
            Some("public".into()),
            &path,
            Range::default(),
        ));
    }
    out
}
