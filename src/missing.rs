use crate::creates::{self, Methods};
use crate::index::{Entry, RefKind};
use crate::php::{self, Kind, tokenize};
use crate::project::{Lines, rel};
use crate::refs::{self, Want};
use crate::{Laravel, indexer};
use openidea_plugin::file_actions::{FileAction, FileOperation};
use openidea_plugin::providers::Document;
use openidea_plugin::{Diagnostic, Error, Project, Severity, TextEdit};
use serde::{Deserialize, Serialize};

pub fn installed_major(installed_json: &str, package: &str) -> Option<u32> {
    let json: serde_json::Value = serde_json::from_str(installed_json).ok()?;
    let list = json.get("packages").unwrap_or(&json).as_array()?;
    let version = list
        .iter()
        .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(package))?
        .get("version")?
        .as_str()?;
    version
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()
}

pub fn defines_names(path: &str) -> bool {
    let path = path.replace('\\', "/");
    [
        "/resources/views/",
        "/resources/js/Pages/",
        "/resources/js/pages/",
        "/resources/ts/Pages/",
        "/app/",
        "/config/",
        "/lang/",
        "/routes/",
    ]
    .iter()
    .any(|dir| path.contains(dir))
}

pub fn requires(composer_json: &str, package: &str) -> bool {
    composer_json.contains(&format!("\"{package}\""))
}

pub fn config_unknown(entries: &[Entry], key: &str) -> bool {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() < 2
        || parts
            .iter()
            .any(|p| p.is_empty() || p.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    let exists = |k: &str| entries.iter().any(|e| e.key == k);
    if exists(key) || !exists(parts[0]) {
        return false;
    }
    let mut parent = parts[0].to_owned();
    for part in &parts[1..parts.len() - 1] {
        let next = format!("{parent}.{part}");
        if !exists(&next) {
            break;
        }
        parent = next;
    }
    let prefix = format!("{parent}.");
    parent == parts[0] || entries.iter().any(|e| e.key.starts_with(&prefix))
}

pub fn livewire_tags(text: &str) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find("<livewire:") {
        let start = from + i + "<livewire:".len();
        let end = text[start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':')))
            .map_or(text.len(), |n| start + n);
        let name = &text[start..end];
        if !name.is_empty() && !name.contains("::") && !matches!(name, "styles" | "scripts") {
            out.push((name.to_owned(), start, end));
        }
        from = end.max(start);
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub start: usize,
    pub end: usize,
    pub class: String,
    pub path: String,
    pub methods: Vec<String>,
}

fn statement_span(text: &str, from: usize) -> (usize, usize) {
    let start = text[..from].rfind(';').map_or(0, |i| i + 1);
    let end = text[from..].find(';').map_or(text.len(), |i| from + i);
    (start, end)
}

fn class_fqn(text: &str, short: &str) -> String {
    let (namespace, imports) = indexer::imports(text);
    if let Some(full) = imports.get(short) {
        return full.clone();
    }
    let tokens = tokenize(text, 0, text.len());
    for (i, t) in tokens.iter().enumerate() {
        if let Kind::Ident(name) = &t.kind
            && name.trim_start_matches('\\').rsplit('\\').next() == Some(short)
            && name.contains('\\')
            && matches!(tokens.get(i + 1).map(|n| &n.kind), Some(Kind::DoubleColon))
        {
            return name.trim_start_matches('\\').to_owned();
        }
    }
    if namespace.is_empty() {
        format!("App\\Http\\Controllers\\{short}")
    } else {
        format!("{namespace}\\{short}")
    }
}

fn find_literal(text: &str, from: usize, to: usize, value: &str) -> Option<(usize, usize)> {
    let span = text.get(from..to)?;
    for quote in ['\'', '"'] {
        let needle = format!("{quote}{value}{quote}");
        if let Some(i) = span.find(&needle) {
            let start = from + i + 1;
            return Some((start, start + value.len()));
        }
    }
    None
}

pub fn missing_methods(
    text: &str,
    psr4: &[(String, String)],
    read: &dyn Fn(&str) -> Option<String>,
) -> Vec<Missing> {
    let path_of = |fqn: &str| -> Option<String> {
        psr4.iter().find_map(|(ns, dir)| {
            let rest = fqn.strip_prefix(ns.as_str())?;
            Some(format!(
                "{}/{}.php",
                dir.trim_end_matches('/'),
                rest.replace('\\', "/")
            ))
        })
    };
    let reader = |fqn: &str| -> Option<(String, String)> {
        let path = path_of(fqn)?;
        let text = read(&path)?;
        let namespace = fqn
            .rsplit_once('\\')
            .map(|(n, _)| n.to_owned())
            .unwrap_or_default();
        Some((text, namespace))
    };
    let mut out: Vec<Missing> = Vec::new();
    for route in crate::index::parse_routes(text) {
        let Some((short, method)) = &route.action else {
            continue;
        };
        let fqn = class_fqn(text, short);
        let Some(path) = path_of(&fqn) else { continue };
        let Methods::Known(known) = creates::class_methods(&fqn, &reader, 0) else {
            continue;
        };
        if known.iter().any(|m| m.eq_ignore_ascii_case(method)) {
            continue;
        }
        let resource = route
            .detail
            .as_deref()
            .is_some_and(|d| d.starts_with("resource "));
        let (start, end) = if resource {
            (route.start, route.end)
        } else {
            let after = route.uri.map_or(route.start, |(_, e)| e);
            let (_, stop) = statement_span(text, after);
            if method == "__invoke" {
                find_class_ref(text, after, stop, short)
                    .unwrap_or(route.uri.unwrap_or((route.start, route.end)))
            } else {
                find_literal(text, after, stop, method)
                    .unwrap_or(route.uri.unwrap_or((route.start, route.end)))
            }
        };
        if let Some(m) = out
            .iter_mut()
            .find(|m| m.start == start && m.end == end && m.path == path)
        {
            if !m.methods.contains(method) {
                m.methods.push(method.clone());
            }
            continue;
        }
        out.push(Missing {
            start,
            end,
            class: short.clone(),
            path,
            methods: vec![method.clone()],
        });
    }
    out
}

fn find_class_ref(text: &str, from: usize, to: usize, short: &str) -> Option<(usize, usize)> {
    let span = text.get(from..to)?;
    let i = span.find(&format!("{short}::class"))?;
    Some((from + i, from + i + short.len()))
}

pub fn message(m: &Missing) -> String {
    let list: Vec<String> = m.methods.iter().map(|x| format!("{x}()")).collect();
    match list.as_slice() {
        [one] => format!("Method {}::{one} not found", m.class),
        _ => format!("{} has no {}", m.class, list.join(", ")),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "create")]
pub enum CreateRef {
    View {
        name: String,
    },
    Livewire {
        name: String,
    },
    Inertia {
        name: String,
    },
    Methods {
        path: String,
        class: String,
        methods: Vec<String>,
    },
}

impl CreateRef {
    pub fn title(&self) -> String {
        match self {
            Self::View { name } => format!(
                "Create view '{name}' ({})",
                creates::view_path(name).unwrap_or_default()
            ),
            Self::Livewire { name } => format!(
                "Create Livewire component '{name}' ({})",
                creates::livewire(name)
                    .map(|l| l.class_path)
                    .unwrap_or_default()
            ),
            Self::Inertia { name } => format!("Create Inertia page '{name}'"),
            Self::Methods { class, methods, .. } => {
                let list: Vec<String> = methods.iter().map(|m| format!("{m}()")).collect();
                format!("Create {class}::{}", list.join(", "))
            }
        }
    }
}

impl Laravel {
    pub(crate) fn livewire_checked(&self, project: &Project) -> bool {
        let composer = project.read_file("composer.json").unwrap_or_default();
        if !requires(&composer, "livewire/livewire") {
            return false;
        }
        project
            .read_file("vendor/composer/installed.json")
            .ok()
            .and_then(|j| installed_major(&j, "livewire/livewire"))
            .is_some_and(|major| major <= 3)
    }

    pub(crate) fn inertia_checked(&mut self, project: &Project, root: &str) -> bool {
        let composer = project.read_file("composer.json").unwrap_or_default();
        requires(&composer, "inertiajs/inertia-laravel")
            && !self
                .cache
                .get(RefKind::InertiaPage, project, root)
                .is_empty()
    }

    pub(crate) fn missing_diagnostics(
        &mut self,
        project: &Project,
        root: &str,
        path: &str,
        text: &str,
    ) -> Vec<Diagnostic> {
        let own = rel(root, path).replace('\\', "/");
        let lines = Lines::new(text);
        let mut out = Vec::new();
        if own.ends_with(".blade.php")
            && openidea_plugin::host::setting_bool("unknownLivewireInspection").unwrap_or(true)
            && self.livewire_checked(project)
        {
            let known = self.cache.get(RefKind::Livewire, project, root);
            for (name, start, end) in livewire_tags(text) {
                if !known.iter().any(|e| e.key == name) {
                    out.push(
                        Diagnostic::new(
                            lines.range(text, start, end),
                            Severity::Warning,
                            format!("Livewire component '{name}' not found"),
                        )
                        .code("laravel.unknown-livewire-component"),
                    );
                }
            }
        }
        if own.starts_with("routes/")
            && !own.ends_with(".blade.php")
            && openidea_plugin::host::setting_bool("missingControllerMethodInspection")
                .unwrap_or(true)
        {
            for m in self.controller_gaps(project, text) {
                out.push(
                    Diagnostic::new(
                        lines.range(text, m.start, m.end),
                        Severity::Warning,
                        message(&m),
                    )
                    .code("laravel.missing-controller-method"),
                );
            }
        }
        out
    }

    fn controller_gaps(&self, project: &Project, text: &str) -> Vec<Missing> {
        let composer = project.read_file("composer.json").unwrap_or_default();
        let psr4 = crate::casts::psr4(&composer);
        let read = |p: &str| project.read_file(p).ok();
        missing_methods(text, &psr4, &read)
    }

    pub(crate) fn create_at(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<CreateRef> {
        let own = rel(root, &doc.path).replace('\\', "/");
        let blade = own.ends_with(".blade.php");
        if own.starts_with("routes/") && !blade {
            return self
                .controller_gaps(project, &doc.text)
                .into_iter()
                .find(|m| m.start <= cursor && cursor <= m.end)
                .map(|m| CreateRef::Methods {
                    path: m.path,
                    class: m.class,
                    methods: m.methods,
                });
        }
        if blade
            && let Some((name, ..)) = livewire_tags(&doc.text)
                .into_iter()
                .find(|(_, s, e)| *s <= cursor && cursor <= *e)
        {
            let known = self.cache.get(RefKind::Livewire, project, root);
            return (!known.iter().any(|e| e.key == name) && creates::livewire(&name).is_some())
                .then_some(CreateRef::Livewire { name });
        }
        let arg = php::arg_at(&doc.text, blade, cursor)?;
        if !arg.closed || arg.concat {
            return None;
        }
        let seg = refs::segment(
            &arg,
            cursor.checked_sub(arg.start)?,
            &crate::scope_of(&doc.text),
        )?;
        let Want::Kind(kind) = seg.want else {
            return None;
        };
        let name = arg.value.get(seg.start..seg.end)?.to_owned();
        if !refs::checkable(kind, &name) {
            return None;
        }
        let known = |entries: &[Entry]| entries.iter().any(|e| e.key == name);
        match kind {
            RefKind::View => {
                let views = self.cache.get(RefKind::View, project, root);
                (!known(&views) && creates::view_path(&name).is_some())
                    .then_some(CreateRef::View { name })
            }
            RefKind::Livewire => {
                let items = self.cache.get(RefKind::Livewire, project, root);
                (!known(&items) && creates::livewire(&name).is_some())
                    .then_some(CreateRef::Livewire { name })
            }
            RefKind::InertiaPage => {
                let pages = self.cache.get(RefKind::InertiaPage, project, root);
                (!known(&pages) && name.split('/').all(|s| !s.is_empty()))
                    .then_some(CreateRef::Inertia { name })
            }
            _ => None,
        }
    }

    pub(crate) fn create_action(
        &mut self,
        project: &Project,
        root: &str,
        doc: &Document,
        cursor: usize,
    ) -> Option<FileAction> {
        let target = self.create_at(project, root, doc, cursor)?;
        Some(FileAction {
            title: target.title(),
            kind: Some("quickfix".into()),
            data: serde_json::to_string(&target).ok(),
            ..FileAction::default()
        })
    }

    pub(crate) fn resolve_create(
        &mut self,
        project: &Project,
        root: &str,
        target: CreateRef,
        action: FileAction,
    ) -> Result<FileAction, Error> {
        let create = |path: String, text: String| FileOperation::Create {
            path,
            text,
            overwrite: false,
        };
        match target {
            CreateRef::View { name } => {
                let path = creates::view_path(&name).ok_or("not a view name a file can have")?;
                Ok(FileAction {
                    operations: vec![create(path.clone(), creates::view_stub())],
                    open: Some(path),
                    data: None,
                    ..action
                })
            }
            CreateRef::Livewire { name } => {
                let lw = creates::livewire(&name).ok_or("not a Livewire component name")?;
                let mut operations = vec![create(lw.class_path.clone(), lw.class_text)];
                if project.read_file(&lw.view_path).is_err() {
                    operations.push(create(lw.view_path, lw.view_text));
                }
                Ok(FileAction {
                    operations,
                    open: Some(lw.class_path),
                    data: None,
                    ..action
                })
            }
            CreateRef::Inertia { name } => {
                let existing: Vec<String> = self
                    .cache
                    .get(RefKind::InertiaPage, project, root)
                    .iter()
                    .filter_map(|e| e.detail.clone())
                    .collect();
                let package = project.read_file("package.json").unwrap_or_default();
                let (dir, ext) = creates::page_style(&existing, &package);
                let (path, text) =
                    creates::inertia_page(&name, &dir, &ext).ok_or("not an Inertia page name")?;
                Ok(FileAction {
                    operations: vec![create(path.clone(), text)],
                    open: Some(path),
                    data: None,
                    ..action
                })
            }
            CreateRef::Methods { path, methods, .. } => {
                let text = project
                    .read_file(&path)
                    .map_err(|e| Error::Failed(format!("reading {path}: {e}")))?;
                let edits = creates::add_methods(&text, &methods)
                    .ok_or("couldn't find the end of the class")?;
                let lines = Lines::new(&text);
                let edits = edits
                    .into_iter()
                    .map(|(s, e, t)| TextEdit {
                        range: lines.range(&text, s, e),
                        text: t,
                    })
                    .collect();
                Ok(FileAction {
                    edits: vec![(path.clone(), edits)],
                    open: Some(path),
                    data: None,
                    ..action
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::Range;

    fn entry(key: &str) -> Entry {
        Entry::new(key, None, "/p/config/x.php", Range::default())
    }

    #[test]
    fn config_keys_are_unknown_only_under_a_known_literal_array() {
        let entries: Vec<Entry> = [
            "app",
            "app.name",
            "app.providers",
            "app.providers.a",
            "cache",
            "cache.stores",
        ]
        .iter()
        .map(|k| entry(k))
        .collect();
        assert!(config_unknown(&entries, "app.nope"));
        assert!(config_unknown(&entries, "app.providers.b"));
        assert!(!config_unknown(&entries, "app.name"));
        assert!(
            !config_unknown(&entries, "sanctum.guard"),
            "unknown file: a package may merge it"
        );
        assert!(
            !config_unknown(&entries, "cache.stores.redis"),
            "cache.stores has no literal children"
        );
        assert!(!config_unknown(&entries, "app.providers.0"));
        assert!(!config_unknown(&entries, "app"));
    }

    #[test]
    fn reads_installed_versions() {
        let v2 = "{\"packages\": [{\"name\": \"livewire/livewire\", \"version\": \"v3.6.1\"}]}";
        assert_eq!(installed_major(v2, "livewire/livewire"), Some(3));
        let v1 = "[{\"name\": \"livewire/livewire\", \"version\": \"4.0.0\"}]";
        assert_eq!(installed_major(v1, "livewire/livewire"), Some(4));
        assert_eq!(installed_major(v2, "inertiajs/inertia-laravel"), None);
    }

    #[test]
    fn finds_livewire_tags() {
        let text = "<livewire:styles />\n<livewire:user-table :rows=\"$rows\" />\n<livewire:admin.stats/>\n<livewire:pkg::thing />\n";
        let names: Vec<String> = livewire_tags(text).into_iter().map(|(n, ..)| n).collect();
        assert_eq!(names, ["user-table", "admin.stats"]);
    }

    #[test]
    fn finds_controller_methods_routes_point_to_but_that_do_not_exist() {
        let routes = "<?php\nuse App\\Http\\Controllers\\PhotoController;\nRoute::get('/photos', [PhotoController::class, 'index']);\nRoute::get('/gallery', [PhotoController::class, 'gallery'])->name('gallery');\nRoute::resource('photos', PhotoController::class)->only(['index', 'store', 'destroy']);\nRoute::get('/x', App\\Http\\Controllers\\InvokeController::class);\nRoute::get('/v', [VendorController::class, 'nope']);\n";
        let files = std::collections::BTreeMap::from([
            (
                "app/Http/Controllers/PhotoController.php",
                "<?php\nnamespace App\\Http\\Controllers;\nclass PhotoController extends Controller\n{\n    public function index() {}\n}\n",
            ),
            (
                "app/Http/Controllers/Controller.php",
                "<?php\nnamespace App\\Http\\Controllers;\nabstract class Controller\n{\n}\n",
            ),
            (
                "app/Http/Controllers/InvokeController.php",
                "<?php\nnamespace App\\Http\\Controllers;\nclass InvokeController\n{\n}\n",
            ),
        ]);
        let read = |p: &str| files.get(p).map(|t| (*t).to_owned());
        let psr4 = vec![("App\\".to_owned(), "app/".to_owned())];
        let found = missing_methods(routes, &psr4, &read);
        let got: Vec<(String, String)> = found
            .iter()
            .map(|m| (routes[m.start..m.end].to_owned(), message(m)))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "gallery".to_owned(),
                    "Method PhotoController::gallery() not found".to_owned()
                ),
                (
                    "photos".to_owned(),
                    "PhotoController has no store(), destroy()".to_owned()
                ),
                (
                    "InvokeController".to_owned(),
                    "Method InvokeController::__invoke() not found".to_owned()
                ),
            ]
        );
        assert_eq!(found[1].path, "app/Http/Controllers/PhotoController.php");
    }

    #[test]
    fn create_titles_name_the_file() {
        assert_eq!(
            CreateRef::View {
                name: "orders.show".into()
            }
            .title(),
            "Create view 'orders.show' (resources/views/orders/show.blade.php)"
        );
        assert_eq!(
            CreateRef::Methods {
                path: "x".into(),
                class: "PhotoController".into(),
                methods: vec!["store".into(), "destroy".into()]
            }
            .title(),
            "Create PhotoController::store(), destroy()"
        );
    }
}
