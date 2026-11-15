use crate::index::RefKind;
use crate::php;
use crate::refs::{self, Want};

pub const ROUTE: &str = "laravel-route#";
pub const CONFIG: &str = "laravel-config#";
pub const TRANSLATION: &str = "laravel-trans#";

pub type Found = (String, usize, usize);

pub fn last_segment(key: &str) -> &str {
    key.rsplit('.').next().unwrap_or(key)
}

pub fn prefix(kind: RefKind) -> Option<&'static str> {
    match kind {
        RefKind::Route => Some(ROUTE),
        RefKind::Config => Some(CONFIG),
        RefKind::Translation => Some(TRANSLATION),
        _ => None,
    }
}

fn literal(text: &str, start: usize, end: usize) -> Option<(usize, &str)> {
    let b = text.as_bytes();
    let quote = *b.get(start.checked_sub(1)?)?;
    if !matches!(quote, b'\'' | b'"') || b.get(end) != Some(&quote) {
        return None;
    }
    let inner = text.get(start..end)?;
    (!inner.contains(['\\', '$'])).then_some((start, inner))
}

pub fn route_definitions(text: &str) -> Vec<Found> {
    crate::index::routes::parse(text)
        .into_iter()
        .filter(|r| {
            !r.detail
                .as_deref()
                .is_some_and(|d| d.starts_with("resource "))
        })
        .filter_map(|r| {
            let name = r.name?;
            let (at, inner) = literal(text, r.start, r.end)?;
            let last = last_segment(&name);
            if last.is_empty() || !inner.ends_with(last) || !name.ends_with(inner) {
                return None;
            }
            let end = at + inner.len();
            Some((format!("{ROUTE}{name}"), end - last.len(), end))
        })
        .collect()
}

pub fn array_definitions(key_prefix: &str, file: &str, text: &str) -> Vec<Found> {
    let tokens = php::tokenize(text, 0, text.len());
    php::array_keys(&tokens)
        .into_iter()
        .filter_map(|k| {
            let (at, inner) = literal(text, k.start, k.end)?;
            if inner.is_empty() || inner.contains('.') || last_segment(&k.path) != inner {
                return None;
            }
            Some((
                format!("{key_prefix}{file}.{}", k.path),
                at,
                at + inner.len(),
            ))
        })
        .collect()
}

pub fn config_file(rel_path: &str) -> Option<String> {
    let name = rel_path.strip_prefix("config/")?.strip_suffix(".php")?;
    (!name.is_empty()).then(|| name.replace('/', "."))
}

pub fn translation_file(rel_path: &str) -> Option<String> {
    let rest = rel_path.strip_prefix("resources/").unwrap_or(rel_path);
    let (locale, file) = rest.strip_prefix("lang/")?.split_once('/')?;
    if locale == "vendor" || locale.is_empty() {
        return None;
    }
    let file = file.strip_suffix(".php")?;
    (!file.is_empty()).then(|| file.replace('/', "."))
}

pub fn references(
    text: &str,
    blade: bool,
    scope: &refs::Scope,
    mut defined: impl FnMut(RefKind, &str) -> bool,
) -> Vec<Found> {
    let mut out = Vec::new();
    for arg in php::args_in(text, blade) {
        if !arg.closed || arg.concat {
            continue;
        }
        for seg in refs::segments(&arg, scope) {
            let Want::Kind(kind) = seg.want else { continue };
            let Some(key_prefix) = prefix(kind) else {
                continue;
            };
            let Some(key) = arg.value.get(seg.start..seg.end) else {
                continue;
            };
            if !refs::checkable(kind, key) || key.contains("::") {
                continue;
            }
            let last = last_segment(key);
            let end = arg.start + seg.end;
            let start = end - last.len();
            if last.is_empty() || text.get(arg.start + seg.start..end) != Some(key) {
                continue;
            }
            if !defined(kind, key) {
                continue;
            }
            out.push((format!("{key_prefix}{key}"), start, end));
        }
    }
    out
}

pub fn route_literal_holds(name: &str, width: u32) -> bool {
    let last = last_segment(name).encode_utf16().count();
    last > 0 && width as usize >= last
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans<'a>(text: &'a str, found: &'a [Found]) -> Vec<(&'a str, &'a str)> {
        found
            .iter()
            .map(|(k, s, e)| (k.as_str(), &text[*s..*e]))
            .collect()
    }

    #[test]
    fn anchors_route_names_on_their_last_segment() {
        let text = "<?php\nRoute::get('/', HomeController::class)->name('home');\nRoute::name('admin.')->group(function () {\n    Route::post('/users', [UserController::class, 'store'])->name('users.store');\n    Route::name('reports.')->group(function () {\n        Route::get('/daily', DailyController::class)->name('daily');\n    });\n});\nRoute::resource('photos', PhotoController::class);\n";
        let found = route_definitions(text);
        assert_eq!(
            spans(text, &found),
            [
                ("laravel-route#home", "home"),
                ("laravel-route#admin.users.store", "store"),
                ("laravel-route#admin.reports.daily", "daily"),
            ]
        );
    }

    #[test]
    fn skips_route_names_whose_prefix_has_no_dot() {
        let text = "<?php\nRoute::name('admin')->group(function () {\n    Route::get('/u', UserController::class)->name('users');\n});\n";
        assert!(route_definitions(text).is_empty());
    }

    #[test]
    fn anchors_config_and_translation_keys() {
        let text = "<?php\nreturn [\n    'name' => env('APP_NAME', 'Laravel'),\n    'mail' => [\n        'from' => 'x',\n    ],\n    'a.b' => 1,\n];\n";
        let found = array_definitions(CONFIG, "app", text);
        assert_eq!(
            spans(text, &found),
            [
                ("laravel-config#app.name", "name"),
                ("laravel-config#app.mail", "mail"),
                ("laravel-config#app.mail.from", "from"),
            ]
        );
    }

    #[test]
    fn names_config_and_translation_files() {
        assert_eq!(config_file("config/app.php").as_deref(), Some("app"));
        assert_eq!(
            config_file("config/services/aws.php").as_deref(),
            Some("services.aws")
        );
        assert_eq!(config_file("app/config.php"), None);
        assert_eq!(
            translation_file("lang/en/messages.php").as_deref(),
            Some("messages")
        );
        assert_eq!(
            translation_file("resources/lang/pt_BR/auth.php").as_deref(),
            Some("auth")
        );
        assert_eq!(translation_file("lang/en.json"), None);
        assert_eq!(translation_file("lang/vendor/pkg/en/x.php"), None);
    }

    #[test]
    fn anchors_defined_usages_on_their_last_segment() {
        let text = "<?php\nreturn [\n    route('admin.users.store'),\n    config('app.name'),\n    __('messages.welcome'),\n    route('photos.index'),\n    route('admin.' . $x),\n    config(\"app.$key\"),\n    __('pkg::file.key'),\n];\n";
        let scope = refs::Scope::default();
        let found = references(text, false, &scope, |kind, key| {
            !(kind == RefKind::Route && key == "photos.index")
        });
        assert_eq!(
            spans(text, &found),
            [
                ("laravel-route#admin.users.store", "store"),
                ("laravel-config#app.name", "name"),
                ("laravel-trans#messages.welcome", "welcome"),
            ]
        );
    }

    #[test]
    fn anchors_usages_in_blade() {
        let text =
            "<a href=\"{{ route('home') }}\">{{ __('messages.hi') }}</a>\n@lang('messages.bye')\n";
        let found = references(text, true, &refs::Scope::default(), |_, _| true);
        let names: Vec<&str> = spans(text, &found).into_iter().map(|(k, _)| k).collect();
        assert!(names.contains(&"laravel-route#home"), "{names:?}");
        assert!(names.contains(&"laravel-trans#messages.hi"), "{names:?}");
    }

    #[test]
    fn a_route_literal_must_hold_the_last_segment() {
        assert!(route_literal_holds("admin.reports.daily", 5));
        assert!(!route_literal_holds("adminusers", 5));
        assert!(route_literal_holds("home", 4));
    }
}
