use crate::index::{Entry, RefKind};
use crate::project::{find, read_listed, rel};
use crate::sections::Row;
use openidea_plugin::Project;
use std::collections::{BTreeMap, BTreeSet};

pub const COLUMNS: [&str; 4] = ["Kind", "Name", "Defined in", "Why"];

const PHP_GLOBS: [&str; 7] = [
    "app/**/*.php",
    "routes/**/*.php",
    "database/**/*.php",
    "tests/**/*.php",
    "config/**/*.php",
    "bootstrap/app.php",
    "resources/views/**/*.blade.php",
];

const JS_GLOBS: [&str; 6] = [
    "resources/js/**/*.js",
    "resources/js/**/*.ts",
    "resources/js/**/*.jsx",
    "resources/js/**/*.tsx",
    "resources/js/**/*.vue",
    "resources/js/**/*.svelte",
];

const FRAMEWORK_ROUTES: [&str; 13] = [
    "login",
    "logout",
    "register",
    "password.request",
    "password.reset",
    "password.email",
    "password.update",
    "password.confirm",
    "verification.notice",
    "verification.verify",
    "verification.send",
    "home",
    "dashboard",
];

pub const PACKAGE_ROUTE_PREFIXES: [&str; 10] = [
    "sanctum.",
    "livewire.",
    "ignition.",
    "horizon.",
    "telescope.",
    "filament.",
    "pulse.",
    "storage.",
    "boost.",
    "passport.",
];

const FRAMEWORK_LANG_GROUPS: [&str; 4] = ["auth", "pagination", "passwords", "validation"];

const KNOWN_CONFIG: [&str; 52] = [
    "app",
    "auth",
    "broadcasting",
    "cache",
    "concurrency",
    "cors",
    "database",
    "filesystems",
    "hashing",
    "logging",
    "mail",
    "queue",
    "sanctum",
    "services",
    "session",
    "view",
    "reverb",
    "horizon",
    "telescope",
    "pulse",
    "octane",
    "scout",
    "cashier",
    "fortify",
    "jetstream",
    "livewire",
    "inertia",
    "permission",
    "debugbar",
    "ide-helper",
    "l5-swagger",
    "excel",
    "media-library",
    "activitylog",
    "backup",
    "tinker",
    "trustedproxy",
    "image",
    "dompdf",
    "sentry",
    "filament",
    "nova",
    "wayfinder",
    "boost",
    "ray",
    "clockwork",
    "data",
    "query-builder",
    "purifier",
    "passport",
    "socialite",
    "folio",
];

#[derive(Default, Debug)]
pub struct Used {
    exact: BTreeMap<RefKind, BTreeSet<String>>,
    prefixes: BTreeMap<RefKind, BTreeSet<String>>,
}

impl Used {
    pub fn add(&mut self, kind: RefKind, value: &str, exact: bool) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let target = if exact {
            &mut self.exact
        } else {
            &mut self.prefixes
        };
        target.entry(kind).or_default().insert(value.to_owned());
    }

    pub fn uses(&self, kind: RefKind, key: &str) -> bool {
        self.exact.get(&kind).is_some_and(|s| s.contains(key))
            || self
                .prefixes
                .get(&kind)
                .is_some_and(|p| p.iter().any(|x| key.starts_with(x.as_str())))
    }

    fn uses_under(&self, kind: RefKind, key: &str) -> bool {
        let dotted = format!("{key}.");
        self.uses(kind, key)
            || self
                .exact
                .get(&kind)
                .is_some_and(|s| s.iter().any(|x| x.starts_with(&dotted)))
    }
}

pub fn component_tags(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("<x-") {
        let name: String = text[at + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '_'))
            .collect();
        if name.is_empty()
            || name == "slot"
            || name.starts_with("slot:")
            || name == "dynamic-component"
        {
            continue;
        }
        out.push(name.replace("::", "."));
    }
    out
}

pub fn js_refs(text: &str) -> Vec<(RefKind, String)> {
    let mut out = Vec::new();
    for (call, kind) in [
        ("route(", RefKind::Route),
        ("__(", RefKind::Translation),
        ("trans(", RefKind::Translation),
        ("$t(", RefKind::Translation),
    ] {
        for (at, _) in text.match_indices(call) {
            if at > 0
                && text[..at]
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                && call != "$t("
            {
                continue;
            }
            let rest = text[at + call.len()..].trim_start();
            let Some(quote) = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '\'' | '"' | '`'))
            else {
                continue;
            };
            if let Some(value) = rest[1..].split(quote).next() {
                out.push((kind, value.to_owned()));
            }
        }
    }
    out
}

pub fn collect(
    project: &Project,
    root: &str,
    php_refs: impl Fn(&str, bool) -> Vec<(RefKind, String, bool)>,
) -> Used {
    let mut used = Used::default();
    let mut paths: Vec<String> = PHP_GLOBS
        .iter()
        .flat_map(|g| find(project, root, g, 20_000))
        .collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        let Ok(text) = read_listed(project, &path) else {
            continue;
        };
        let blade = path.ends_with(".blade.php");
        for (kind, value, exact) in php_refs(&text, blade) {
            used.add(kind, &value, exact);
        }
        if blade {
            for tag in component_tags(&text) {
                used.add(RefKind::View, &format!("components.{tag}"), true);
                used.add(RefKind::View, &format!("components.{tag}.index"), true);
            }
        }
    }
    for glob in JS_GLOBS {
        for path in find(project, root, glob, 20_000) {
            let Ok(text) = read_listed(project, &path) else {
                continue;
            };
            for (kind, value) in js_refs(&text) {
                let exact = !value.contains("${");
                let value = value.split("${").next().unwrap_or_default().to_owned();
                used.add(kind, &value, exact);
            }
        }
    }
    used
}

fn place(root: &str, e: &Entry) -> (String, u32) {
    (
        rel(root, &e.location.path).replace('\\', "/"),
        e.location.range.start.line + 1,
    )
}

fn row(root: &str, kind: &str, name: &str, e: &Entry, why: &str) -> Row {
    let (path, line) = place(root, e);
    Row::at(
        vec![
            kind.into(),
            name.into(),
            format!("{path}:{line}"),
            why.into(),
        ],
        &path,
        line,
    )
}

pub struct Indexes<'a> {
    pub routes: &'a [Entry],
    pub views: &'a [Entry],
    pub translations: &'a [Entry],
    pub config: &'a [Entry],
    pub folio: bool,
}

pub fn rows(root: &str, idx: &Indexes, used: &Used) -> Vec<Row> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for e in idx.routes {
        let path = e.location.path.replace('\\', "/");
        if path.contains("/bootstrap/cache/")
            || path.contains("/vendor/")
            || FRAMEWORK_ROUTES.contains(&e.key.as_str())
            || PACKAGE_ROUTE_PREFIXES.iter().any(|p| e.key.starts_with(p))
            || used.uses(RefKind::Route, &e.key)
            || !seen.insert(("route", e.key.clone()))
        {
            continue;
        }
        out.push(row(
            root,
            "Route",
            &e.key,
            e,
            "No route('…'), to_route('…') or redirect()->route('…') uses this name, in PHP, Blade or resources/js",
        ));
    }
    for e in idx.views {
        let key = e.key.as_str();
        if key.contains("::")
            || ["errors.", "vendor.", "mail.", "emails.vendor."]
                .iter()
                .any(|p| key.starts_with(p))
            || (idx.folio && key.starts_with("pages."))
            || used.uses(RefKind::View, key)
            || !seen.insert(("view", e.key.clone()))
        {
            continue;
        }
        let why = if key.starts_with("components.") {
            "No <x-…> tag, @component or view('…') renders it"
        } else {
            "No view('…'), @include, @extends, Route::view or Mailable renders it"
        };
        out.push(row(root, "View", key, e, why));
    }
    for e in idx.translations {
        let key = e.key.as_str();
        let group = key.split('.').next().unwrap_or_default();
        let php_key = key.contains('.') && !key.contains(' ');
        if (php_key && FRAMEWORK_LANG_GROUPS.contains(&group))
            || used.uses_under(RefKind::Translation, key)
            || !seen.insert(("translation", e.key.clone()))
        {
            continue;
        }
        if php_key
            && idx
                .translations
                .iter()
                .any(|o| o.key.starts_with(&format!("{key}.")))
        {
            continue;
        }
        out.push(row(
            root,
            "Translation",
            key,
            e,
            "No __('…'), trans('…'), @lang or $t('…') uses this key",
        ));
    }
    for e in idx.config {
        let key = e.key.as_str();
        let Some((file, rest)) = key.split_once('.') else {
            continue;
        };
        if rest.contains('.')
            || KNOWN_CONFIG.contains(&file)
            || used.uses(RefKind::Config, file)
            || used.uses_under(RefKind::Config, key)
            || !seen.insert(("config", e.key.clone()))
        {
            continue;
        }
        out.push(row(
            root,
            "Config key",
            key,
            e,
            "No config('…') or Config::get('…') reads it, and nothing reads its file as a whole",
        ));
    }
    out
}

pub fn class_rows(
    root: &str,
    kind: &str,
    items: &[(String, String, u32)],
    used_elsewhere: impl Fn(&str, &str) -> bool,
    why: &str,
) -> Vec<Row> {
    items
        .iter()
        .filter(|(name, path, _)| !used_elsewhere(name, path))
        .map(|(name, path, line)| {
            let relative = rel(root, path).replace('\\', "/");
            Row::at(
                vec![
                    kind.into(),
                    name.clone(),
                    format!("{relative}:{line}"),
                    why.into(),
                ],
                &relative,
                *line,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::{Location, Position, Range};

    fn entry(key: &str, path: &str) -> Entry {
        Entry {
            key: key.into(),
            detail: None,
            location: Location {
                path: path.into(),
                range: Range::new(Position::new(2, 0), Position::new(2, 1)),
            },
        }
    }

    #[test]
    fn reads_component_tags_and_js_references() {
        assert_eq!(
            component_tags(
                "<x-alert type=\"x\"/><x-forms.input /><x-slot:title>t</x-slot:title><x-slot name=\"a\"/><x-mail::button>"
            ),
            ["alert", "forms.input", "mail.button"]
        );
        assert_eq!(
            js_refs(
                "router.visit(route('orders.show', id)); t(__(\"Welcome\")); $t('shop.title'); myroute('no'); route(`orders.${x}`)"
            ),
            [
                (RefKind::Route, "orders.show".to_owned()),
                (RefKind::Route, "orders.${x}".to_owned()),
                (RefKind::Translation, "Welcome".to_owned()),
                (RefKind::Translation, "shop.title".to_owned()),
            ]
        );
    }

    #[test]
    fn lists_routes_views_translations_and_config_nothing_uses() {
        let routes = [
            entry("orders.show", "/p/routes/web.php"),
            entry("orders.export", "/p/routes/web.php"),
            entry("login", "/p/routes/auth.php"),
            entry("horizon.index", "/p/bootstrap/cache/routes-v7.php"),
            entry("reports.daily", "/p/routes/web.php"),
        ];
        let views = [
            entry("welcome", "/p/resources/views/welcome.blade.php"),
            entry("orders.old", "/p/resources/views/orders/old.blade.php"),
            entry(
                "components.alert",
                "/p/resources/views/components/alert.blade.php",
            ),
            entry(
                "components.badge",
                "/p/resources/views/components/badge.blade.php",
            ),
            entry("errors.404", "/p/resources/views/errors/404.blade.php"),
        ];
        let translations = [
            entry("shop.title", "/p/lang/en/shop.php"),
            entry("shop.status", "/p/lang/en/shop.php"),
            entry("shop.status.paid", "/p/lang/en/shop.php"),
            entry("shop.legacy", "/p/lang/en/shop.php"),
            entry("auth.failed", "/p/lang/en/auth.php"),
            entry("Welcome", "/p/lang/en.json"),
        ];
        let config = [
            entry("shop.currency", "/p/config/shop.php"),
            entry("shop.tax", "/p/config/shop.php"),
            entry("shop.tax.rate", "/p/config/shop.php"),
            entry("app.name", "/p/config/app.php"),
            entry("billing.key", "/p/config/billing.php"),
        ];
        let mut used = Used::default();
        used.add(RefKind::Route, "orders.show", true);
        used.add(RefKind::Route, "reports.", false);
        used.add(RefKind::View, "welcome", true);
        used.add(RefKind::View, "components.alert", true);
        used.add(RefKind::Translation, "shop.title", true);
        used.add(RefKind::Translation, "shop.status.", false);
        used.add(RefKind::Translation, "Welcome", true);
        used.add(RefKind::Config, "shop.currency", true);
        used.add(RefKind::Config, "billing", true);
        let rows = rows(
            "/p",
            &Indexes {
                routes: &routes,
                views: &views,
                translations: &translations,
                config: &config,
                folio: false,
            },
            &used,
        );
        let names: Vec<String> = rows
            .iter()
            .map(|r| format!("{}:{} @ {}", r.cells[0], r.cells[1], r.cells[2]))
            .collect();
        assert_eq!(
            names,
            [
                "Route:orders.export @ routes/web.php:3",
                "View:orders.old @ resources/views/orders/old.blade.php:3",
                "View:components.badge @ resources/views/components/badge.blade.php:3",
                "Translation:shop.legacy @ lang/en/shop.php:3",
                "Config key:shop.tax @ config/shop.php:3",
            ]
        );
        assert_eq!(
            rows[0].open.as_ref().map(|t| (t.path.as_str(), t.line)),
            Some(("routes/web.php", 3))
        );
    }
}
