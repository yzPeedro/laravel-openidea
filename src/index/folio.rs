use super::Entry;
use crate::php::{Kind, Token, tokenize};
use crate::project::{Lines, find, rel};
use openidea_plugin::{Location, Project, Range};

const PROVIDER: &str = "app/Providers/FolioServiceProvider.php";
const DEFAULT_DIR: &str = "resources/views/pages";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub uri: String,
    pub name: Option<String>,
    pub location: Location,
}

pub fn mounts(provider: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for call in ["Folio::path(", "Folio::route("] {
        let mut from = 0;
        while let Some(i) = provider[from..].find(call) {
            let start = from + i + call.len();
            let end = provider[start..]
                .find(';')
                .map_or(provider.len(), |e| start + e);
            let stmt = &provider[start..end];
            from = end;
            let Some(dir) = quoted_after(stmt, "resource_path(") else {
                continue;
            };
            let uri = quoted_after(stmt, "uri(")
                .or_else(|| quoted_after(stmt, "uri:"))
                .unwrap_or_else(|| "/".into());
            out.push((format!("resources/{}", dir.trim_matches('/')), uri));
        }
    }
    out
}

fn quoted_after(text: &str, marker: &str) -> Option<String> {
    let rest = &text[text.find(marker)? + marker.len()..];
    let rest = rest.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &rest[1..];
    Some(body[..body.find(quote)?].to_owned())
}

fn segment(seg: &str) -> String {
    let Some(inner) = seg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return seg.to_owned();
    };
    let (rest, many) = match inner.strip_prefix("...") {
        Some(r) => (r, true),
        None => (inner, false),
    };
    let (binding, field) = match rest.split_once(':') {
        Some((b, f)) => (b, Some(f)),
        None => (rest, None),
    };
    let class = binding.rsplit(['.', '\\']).next().unwrap_or(binding);
    let mut chars = class.chars();
    let name = match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect::<String>(),
        None => String::new(),
    };
    let name = match field {
        Some(f) => format!("{name}:{f}"),
        None => name,
    };
    if many {
        format!("{{{name}...}}")
    } else {
        format!("{{{name}}}")
    }
}

pub fn uri_of(relative: &str, prefix: &str) -> String {
    let path = relative.strip_suffix(".blade.php").unwrap_or(relative);
    let mut parts: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(segment)
        .collect();
    if parts.last().is_some_and(|p| p == "index") {
        parts.pop();
    }
    let joined = format!("{}/{}", prefix.trim_end_matches('/'), parts.join("/"));
    let joined = joined.trim_end_matches('/');
    if joined.is_empty() {
        "/".into()
    } else if joined.starts_with('/') {
        joined.to_owned()
    } else {
        format!("/{joined}")
    }
}

pub fn page_name(text: &str) -> Option<(String, usize, usize)> {
    let mut from = 0;
    while let Some(i) = text[from..].find("<?php") {
        let start = from + i + 5;
        let end = text[start..].find("?>").map_or(text.len(), |e| start + e);
        let tokens = tokenize(text, start, end);
        for (k, t) in tokens.iter().enumerate() {
            if !matches!(&t.kind, Kind::Ident(n) if n == "name" || n.ends_with("\\name")) {
                continue;
            }
            let plain = k == 0
                || !matches!(
                    tokens[k - 1].kind,
                    Kind::ObjectOp | Kind::DoubleColon | Kind::Ident(_)
                );
            if !plain || tokens.get(k + 1).map(|t| &t.kind) != Some(&Kind::Open('(')) {
                continue;
            }
            if let Some(Token {
                kind: Kind::Str { value, .. },
                start,
                end,
            }) = tokens.get(k + 2)
            {
                return Some((value.clone(), *start, *end));
            }
        }
        from = end;
    }
    None
}

pub fn pages(project: &Project, root: &str) -> Vec<Page> {
    let provider = project.read_file(PROVIDER).unwrap_or_default();
    let mut mounts = mounts(&provider);
    if mounts.is_empty() {
        let composer = project.read_file("composer.json").unwrap_or_default();
        if !composer.contains("\"laravel/folio\"") {
            return Vec::new();
        }
        mounts.push((DEFAULT_DIR.into(), "/".into()));
    }
    let mut out = Vec::new();
    for (dir, prefix) in mounts {
        for path in find(project, root, &format!("{dir}/**/*.blade.php"), 2000) {
            let relative = rel(root, &path).replace('\\', "/");
            let Some(inside) = relative.strip_prefix(&format!("{dir}/")) else {
                continue;
            };
            if inside.split('/').any(|s| s.starts_with('_')) {
                continue;
            }
            let text = crate::project::read_listed(project, &path).unwrap_or_default();
            let lines = Lines::new(&text);
            let named = page_name(&text);
            let range = match &named {
                Some((_, s, e)) => lines.range(&text, *s, *e),
                None => Range::default(),
            };
            out.push(Page {
                uri: uri_of(inside, &prefix),
                name: named.map(|(n, ..)| n),
                location: Location { path, range },
            });
        }
    }
    out.sort_by(|a, b| a.uri.cmp(&b.uri));
    out
}

pub fn named_routes(project: &Project, root: &str) -> Vec<Entry> {
    pages(project, root)
        .into_iter()
        .filter_map(|p| {
            Some(Entry {
                key: p.name?,
                detail: Some(format!("GET {}", p.uri)),
                location: p.location,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_page_files_to_uris() {
        assert_eq!(uri_of("index.blade.php", "/"), "/");
        assert_eq!(uri_of("users/index.blade.php", "/"), "/users");
        assert_eq!(uri_of("users/[id].blade.php", "/"), "/users/{id}");
        assert_eq!(uri_of("users/[User].blade.php", "/"), "/users/{user}");
        assert_eq!(
            uri_of("posts/[.App.Models.Post:slug].blade.php", "/"),
            "/posts/{post:slug}"
        );
        assert_eq!(uri_of("docs/[...pages].blade.php", "/"), "/docs/{pages...}");
        assert_eq!(uri_of("dashboard.blade.php", "/admin"), "/admin/dashboard");
    }

    #[test]
    fn reads_mounts_from_the_provider() {
        let provider = "<?php\nFolio::path(resource_path('views/pages'))->middleware(['*' => []]);\nFolio::route(resource_path('views/pages/admin'), uri: '/admin');\nFolio::path(resource_path(\"views/shop\"))->uri('/shop');\n";
        assert_eq!(
            mounts(provider),
            [
                ("resources/views/pages".to_owned(), "/".to_owned()),
                ("resources/views/shop".to_owned(), "/shop".to_owned()),
                (
                    "resources/views/pages/admin".to_owned(),
                    "/admin".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn finds_the_page_name() {
        let text = "<?php\nuse function Laravel\\Folio\\name;\n\nname('users.show');\n?>\n<div>{{ $user->name }}</div>\n";
        assert_eq!(
            page_name(text).map(|(n, ..)| n).as_deref(),
            Some("users.show")
        );
        assert_eq!(page_name("<?php $route->name('x'); ?>"), None);
        assert_eq!(page_name("<div>name('x')</div>"), None);
    }
}
