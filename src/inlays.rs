use crate::index::RefKind;
use crate::index::routes::{Route, parse};
use crate::php;
use crate::refs::{self, Want};
use std::collections::BTreeMap;

const MAX_LABEL: usize = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub at: usize,
    pub label: String,
    pub tooltip: Option<String>,
}

pub fn shorten(label: &str) -> String {
    let label = label.replace(['\n', '\r'], " ");
    if label.chars().count() <= MAX_LABEL {
        return label;
    }
    let cut: String = label.chars().take(MAX_LABEL - 1).collect();
    format!("{cut}…")
}

fn statement_end(text: &str, from: usize) -> Option<usize> {
    let rest = text.get(from..)?;
    let semi = rest.find(';')?;
    Some(from + semi + 1)
}

fn normalized(uri: &str) -> String {
    let trimmed = uri.trim_matches('/');
    format!("/{trimmed}")
}

fn literal(text: &str, start: usize, end: usize) -> &str {
    text.get(start..end).unwrap_or_default()
}

pub fn route_definitions(text: &str) -> Vec<Hint> {
    let routes = parse(text);
    let mut resources: BTreeMap<(usize, usize), Vec<&Route>> = BTreeMap::new();
    let mut out = Vec::new();
    for route in &routes {
        let detail = route.detail.as_deref().unwrap_or_default();
        if detail.starts_with("resource ") {
            resources
                .entry((route.start, route.end))
                .or_default()
                .push(route);
            continue;
        }
        let Some((verb, uri)) = detail.split_once(' ') else {
            continue;
        };
        let prefixed_uri = route
            .uri
            .is_some_and(|(s, e)| normalized(literal(text, s, e)) != normalized(uri));
        let prefixed_name = route.name.as_deref().is_some_and(|n| {
            route.uri.is_none_or(|(s, _)| s != route.start)
                && n != literal(text, route.start, route.end)
        });
        if !prefixed_uri && !prefixed_name {
            continue;
        }
        let from = [route.end, route.uri.map_or(0, |(_, e)| e)]
            .into_iter()
            .max()
            .unwrap_or(route.end);
        let Some(at) = statement_end(text, from) else {
            continue;
        };
        let label = match &route.name {
            Some(name) => format!("{verb} {uri} · {name}"),
            None => format!("{verb} {uri}"),
        };
        out.push(Hint {
            at,
            label: shorten(&label),
            tooltip: Some("Full URI and name, with the group's prefixes".into()),
        });
    }
    for ((_, end), group) in resources {
        let Some(first) = group.first() else { continue };
        let uri = first
            .detail
            .as_deref()
            .and_then(|d| d.strip_prefix("resource "))
            .unwrap_or_default();
        let names: Vec<&str> = group.iter().filter_map(|r| r.name.as_deref()).collect();
        let Some((base, _)) = names.first().and_then(|n| n.rsplit_once('.')) else {
            continue;
        };
        let actions: Vec<&str> = names
            .iter()
            .filter_map(|n| n.rsplit_once('.').map(|(_, a)| a))
            .collect();
        let Some(at) = statement_end(text, end) else {
            continue;
        };
        out.push(Hint {
            at,
            label: shorten(&format!("{uri} · {base}.{{{}}}", actions.join(","))),
            tooltip: Some(format!("{} routes", actions.len())),
        });
    }
    out.sort_by_key(|h| h.at);
    out
}

pub fn usages(
    text: &str,
    blade: bool,
    scope: &refs::Scope,
    mut label: impl FnMut(RefKind, &str) -> Option<(String, Option<String>)>,
) -> Vec<Hint> {
    let mut out = Vec::new();
    for arg in php::args_in(text, blade) {
        if !arg.closed || arg.concat || arg.is_key {
            continue;
        }
        let segs = refs::segments(&arg, scope);
        let [seg] = segs.as_slice() else { continue };
        let Want::Kind(kind) = seg.want else { continue };
        if !matches!(
            kind,
            RefKind::Route | RefKind::Config | RefKind::Translation
        ) {
            continue;
        }
        if seg.start != 0 || seg.end != arg.value.len() || !refs::checkable(kind, &arg.value) {
            continue;
        }
        let quote = arg.end;
        if !matches!(text.as_bytes().get(quote), Some(b'\'' | b'"')) {
            continue;
        }
        let Some((text, tooltip)) = label(kind, &arg.value) else {
            continue;
        };
        out.push(Hint {
            at: quote + 1,
            label: shorten(&text),
            tooltip,
        });
    }
    out
}

pub fn route_label(detail: &str) -> Option<String> {
    let head = detail.split(" · ").next().unwrap_or(detail).trim();
    let (verb, uri) = head.split_once(' ')?;
    if verb == "resource" {
        return None;
    }
    Some(format!("{} {uri}", verb.split('|').next().unwrap_or(verb)))
}

pub fn config_label(detail: &str, file: &str) -> Option<String> {
    let detail = detail.trim();
    if detail.is_empty() || detail == file || detail.ends_with(".php") {
        return None;
    }
    Some(format!("= {detail}"))
}

pub fn translation_label(detail: &str) -> Option<(String, String)> {
    let (locale, value) = detail.split_once(": ")?;
    let value = value.trim();
    (value.starts_with('\'') || value.starts_with('"'))
        .then(|| (format!("= {value}"), locale.trim().to_owned()))
}

pub fn method_label(method: &str, details: &[String]) -> Option<String> {
    let mut labels: Vec<String> = Vec::new();
    for detail in details {
        let head = detail.split(" · ").next().unwrap_or(detail).trim();
        let Some((verb, uri)) = head.split_once(' ') else {
            continue;
        };
        let label = if verb == "resource" {
            let Some((verb, uri)) = crate::routetools::resource_route(uri, method) else {
                continue;
            };
            format!("{verb} {uri}")
        } else {
            format!("{} {uri}", verb.split('|').next().unwrap_or(verb))
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    (!labels.is_empty()).then(|| shorten(&labels.join(", ")))
}

pub fn signature_end(text: &str, name_at: usize, body_open: usize) -> usize {
    let head = text.get(name_at..body_open).unwrap_or_default();
    name_at + head.trim_end().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, hints: &[Hint]) -> Vec<(String, String)> {
        hints
            .iter()
            .map(|h| {
                let line = text[..h.at]
                    .rsplit('\n')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                (line, h.label.clone())
            })
            .collect()
    }

    #[test]
    fn route_files_show_what_group_prefixes_make() {
        let text = "<?php\nRoute::get('/', HomeController::class)->name('home');\nRoute::prefix('admin')->name('admin.')->group(function () {\n    Route::get('/users', [UserController::class, 'index'])->name('users.index');\n    Route::post('/users', [UserController::class, 'store']);\n});\nRoute::resource('photos', PhotoController::class)->only(['index', 'show']);\n";
        assert_eq!(
            at(text, &route_definitions(text)),
            [
                (
                    "Route::get('/users', [UserController::class, 'index'])->name('users.index');"
                        .to_owned(),
                    "GET /admin/users · admin.users.index".to_owned()
                ),
                (
                    "Route::post('/users', [UserController::class, 'store']);".to_owned(),
                    "POST /admin/users".to_owned()
                ),
                (
                    "Route::resource('photos', PhotoController::class)->only(['index', 'show']);"
                        .to_owned(),
                    "/photos · photos.{index,show}".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn route_hints_follow_a_closure_to_the_end_of_the_route() {
        let text = "<?php\nRoute::prefix('v1')->group(function () {\n    Route::get('/ping', function () {\n        return 'pong';\n    })->name('ping');\n});\n";
        let hints = route_definitions(text);
        assert_eq!(hints.len(), 1);
        assert!(
            text[..hints[0].at].ends_with("->name('ping');"),
            "{hints:?}"
        );
        assert_eq!(hints[0].label, "GET /v1/ping · ping");
    }

    #[test]
    fn usages_get_their_value_after_the_string() {
        let text = "<?php\nroute('home');\nconfig('app.env', 'x');\n__('messages.hi');\nroute('admin.' . $x);\nview('home');\nconfig(['app.env' => 1]);\n";
        let hints = usages(text, false, &refs::Scope::default(), |kind, key| {
            Some((format!("{kind:?}:{key}"), None))
        });
        let got: Vec<(String, String)> = hints
            .iter()
            .map(|h| {
                (
                    text[..h.at].rsplit('\n').next().unwrap().to_owned(),
                    h.label.clone(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("route('home'".to_owned(), "Route:home".to_owned()),
                ("config('app.env'".to_owned(), "Config:app.env".to_owned()),
                (
                    "__('messages.hi'".to_owned(),
                    "Translation:messages.hi".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn labels_come_from_index_details() {
        assert_eq!(
            route_label("GET|HEAD /users · users.index").as_deref(),
            Some("GET /users")
        );
        assert_eq!(route_label("resource /photos"), None);
        assert_eq!(
            config_label("'local'", "config/app.php").as_deref(),
            Some("= 'local'")
        );
        assert_eq!(
            config_label("env('APP_NAME')", "config/app.php").as_deref(),
            Some("= env('APP_NAME')")
        );
        assert_eq!(config_label("config/app.php", "config/app.php"), None);
        assert_eq!(
            translation_label("en: 'Welcome'"),
            Some(("= 'Welcome'".to_owned(), "en".to_owned()))
        );
        assert_eq!(translation_label("en"), None);
        assert_eq!(
            method_label(
                "show",
                &[
                    "resource /photos · photos.show".to_owned(),
                    "GET /p/{id} · p".to_owned()
                ]
            )
            .as_deref(),
            Some("GET /photos/{photo}, GET /p/{id}")
        );
        assert_eq!(method_label("helper", &[]), None);
        assert_eq!(shorten(&"x".repeat(80)).chars().count(), 60);
    }
}
