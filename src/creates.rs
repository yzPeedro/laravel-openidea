use crate::index::{Class, parse_class};
use crate::php::{Kind, tokenize};
use std::collections::BTreeMap;

fn valid_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn view_path(name: &str) -> Option<String> {
    if name.contains("::") || !name.split('.').all(valid_segment) {
        return None;
    }
    Some(format!(
        "resources/views/{}.blade.php",
        name.replace('.', "/")
    ))
}

pub fn view_stub() -> String {
    "<div>\n</div>\n".to_owned()
}

fn studly(segment: &str) -> String {
    segment
        .split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Livewire {
    pub class_path: String,
    pub class_text: String,
    pub view_path: String,
    pub view_text: String,
}

pub fn livewire(name: &str) -> Option<Livewire> {
    let parts: Vec<&str> = name.split('.').collect();
    if name.contains("::") || !parts.iter().all(|p| valid_segment(p)) {
        return None;
    }
    let studly: Vec<String> = parts.iter().map(|p| studly(p)).collect();
    let class = studly.last()?.clone();
    let namespace = std::iter::once("App\\Livewire".to_owned())
        .chain(studly[..studly.len() - 1].iter().cloned())
        .collect::<Vec<_>>()
        .join("\\");
    let view = format!("livewire.{name}");
    Some(Livewire {
        class_path: format!("app/Livewire/{}.php", studly.join("/")),
        class_text: format!(
            "<?php\n\nnamespace {namespace};\n\nuse Livewire\\Component;\n\nclass {class} extends Component\n{{\n    public function render()\n    {{\n        return view('{view}');\n    }}\n}}\n"
        ),
        view_path: format!("resources/views/livewire/{}.blade.php", parts.join("/")),
        view_text: "<div>\n</div>\n".to_owned(),
    })
}

pub const PAGE_DIRS: [&str; 3] = [
    "resources/js/Pages",
    "resources/js/pages",
    "resources/ts/Pages",
];

pub fn inertia_page(name: &str, dir: &str, ext: &str) -> Option<(String, String)> {
    if !name.split('/').all(valid_segment) {
        return None;
    }
    let component = name.rsplit('/').next()?.replace('-', "");
    let text = match ext {
        "vue" => {
            "<script setup>\n</script>\n\n<template>\n    <div></div>\n</template>\n".to_owned()
        }
        "svelte" => "<script>\n</script>\n\n<div></div>\n".to_owned(),
        "tsx" | "jsx" => {
            format!("export default function {component}() {{\n    return <div></div>;\n}}\n")
        }
        _ => return None,
    };
    Some((format!("{dir}/{name}.{ext}"), text))
}

pub fn page_style(existing: &[String], package_json: &str) -> (String, String) {
    let mut dirs: BTreeMap<&str, usize> = BTreeMap::new();
    let mut exts: BTreeMap<&str, usize> = BTreeMap::new();
    for file in existing {
        if let Some(dir) = PAGE_DIRS
            .iter()
            .find(|d| file.starts_with(&format!("{d}/")))
        {
            *dirs.entry(dir).or_default() += 1;
        }
        if let Some((_, ext)) = file.rsplit_once('.') {
            *exts.entry(ext).or_default() += 1;
        }
    }
    let dir = dirs
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map_or(PAGE_DIRS[0], |(d, _)| d);
    let ext = exts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(e, _)| e)
        .unwrap_or_else(|| {
            if package_json.contains("\"@inertiajs/vue3\"") || package_json.contains("\"vue\"") {
                "vue"
            } else if package_json.contains("\"@inertiajs/svelte\"") {
                "svelte"
            } else if package_json.contains("\"typescript\"") {
                "tsx"
            } else {
                "jsx"
            }
        });
    (dir.to_owned(), ext.to_owned())
}

pub fn signature(method: &str) -> &'static str {
    match method {
        "store" => "store(Request $request)",
        "show" => "show(string $id)",
        "edit" => "edit(string $id)",
        "update" => "update(Request $request, string $id)",
        "destroy" => "destroy(string $id)",
        "__invoke" => "__invoke(Request $request)",
        "index" => "index()",
        "create" => "create()",
        _ => "",
    }
}

fn method_stub(method: &str, request: &str) -> String {
    let head = match signature(method) {
        "" => format!("{method}()"),
        s => s.replace("Request $", &format!("{request} $")),
    };
    format!("    public function {head}\n    {{\n    }}\n")
}

pub fn needs_request(methods: &[String]) -> bool {
    methods.iter().any(|m| signature(m).contains("Request "))
}

const HTTP_REQUEST: &str = "Illuminate\\Http\\Request";

/// How the new methods name `Illuminate\Http\Request`, and whether a `use`
/// line must be added for it. An alias the file already has is reused. When
/// the short name `Request` belongs to another class (a facade import, an
/// alias, the class itself), the fully qualified name is written instead: a
/// second `use … Request;` would not compile.
pub fn request_name(text: &str) -> (String, bool) {
    let (_, imports) = crate::indexer::imports(text);
    let is_http = |f: &str| {
        f.trim_start_matches('\\')
            .eq_ignore_ascii_case(HTTP_REQUEST)
    };
    if let Some((alias, _)) = imports.iter().find(|(_, f)| is_http(f)) {
        return (alias.clone(), false);
    }
    let taken = imports.keys().any(|a| a.eq_ignore_ascii_case("Request"))
        || parse_class(text).is_some_and(|c| c.name.eq_ignore_ascii_case("Request"));
    if taken {
        (format!("\\{HTTP_REQUEST}"), false)
    } else {
        ("Request".to_owned(), true)
    }
}

pub fn class_body(text: &str, class: &Class) -> Option<(usize, usize)> {
    let tokens = tokenize(text, class.end, text.len());
    let open = tokens.iter().position(|t| t.kind == Kind::Open('{'))?;
    let mut depth = 0usize;
    for t in &tokens[open..] {
        match t.kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => {
                depth -= 1;
                if depth == 0 {
                    return Some((tokens[open].start, t.start));
                }
            }
            _ => {}
        }
    }
    None
}

pub fn add_methods(text: &str, methods: &[String]) -> Option<Vec<(usize, usize, String)>> {
    let class = parse_class(text)?;
    let (_, close) = class_body(text, &class)?;
    let line_start = text[..close].rfind('\n').map_or(0, |i| i + 1);
    let before = text[..line_start].trim_end();
    let blank = if before.ends_with('{') { "" } else { "\n" };
    let (request, import) = if needs_request(methods) {
        request_name(text)
    } else {
        (String::new(), false)
    };
    let stubs: Vec<String> = methods.iter().map(|m| method_stub(m, &request)).collect();
    let mut edits = vec![(
        line_start,
        line_start,
        format!("{blank}{}", stubs.join("\n")),
    )];
    if import {
        let (at, first) = use_insertion(text)?;
        let line = if first {
            "use Illuminate\\Http\\Request;\n\n"
        } else {
            "use Illuminate\\Http\\Request;\n"
        };
        edits.insert(0, (at, at, line.to_owned()));
    }
    Some(edits)
}

fn use_insertion(text: &str) -> Option<(usize, bool)> {
    let tokens = tokenize(text, 0, text.len());
    let mut last_use_end = None;
    let mut namespace_end = None;
    let mut depth = 0usize;
    for (i, t) in tokens.iter().enumerate() {
        match &t.kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => depth = depth.saturating_sub(1),
            Kind::Ident(k) if depth == 0 && (k == "use" || k == "namespace") => {
                let end = tokens[i..].iter().find(|x| x.kind == Kind::Semicolon)?.end;
                if k == "use" {
                    last_use_end = Some(end);
                } else {
                    namespace_end = Some(end);
                }
            }
            Kind::Ident(k) if depth == 0 && (k == "class" || k == "final" || k == "abstract") => {
                break;
            }
            _ => {}
        }
    }
    let after = last_use_end.or(namespace_end)?;
    let line_end = text[after..]
        .find('\n')
        .map_or(text.len(), |n| after + n + 1);
    Some(if last_use_end.is_some() {
        (line_end, false)
    } else {
        let next = text[line_end..]
            .find('\n')
            .map_or(text.len(), |n| line_end + n + 1);
        if text[line_end..next].trim().is_empty() {
            (next, true)
        } else {
            (line_end, true)
        }
    })
}

pub fn uses_traits(text: &str, class: &Class) -> bool {
    let Some((open, close)) = class_body(text, class) else {
        return false;
    };
    let tokens = tokenize(text, open, close + 1);
    let mut depth = 0usize;
    for (i, t) in tokens.iter().enumerate() {
        match &t.kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => depth = depth.saturating_sub(1),
            Kind::Ident(k) if depth == 1 && k == "use" => {
                let next = tokens.get(i + 1).map(|n| &n.kind);
                if matches!(next, Some(Kind::Ident(_))) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

pub enum Methods {
    Known(Vec<String>),
    Unknown,
}

pub fn class_methods(
    fqn: &str,
    read: &dyn Fn(&str) -> Option<(String, String)>,
    depth: usize,
) -> Methods {
    let Some((text, namespace_of_file)) = read(fqn) else {
        return Methods::Unknown;
    };
    let Some(class) = parse_class(&text) else {
        return Methods::Unknown;
    };
    if uses_traits(&text, &class) {
        return Methods::Unknown;
    }
    let mut methods: Vec<String> = class
        .methods
        .iter()
        .filter(|m| m.public)
        .map(|m| m.name.clone())
        .collect();
    if let Some(parent) = &class.extends {
        if depth >= 4 {
            return Methods::Unknown;
        }
        let (namespace, imports) = crate::indexer::imports(&text);
        let namespace = if namespace.is_empty() {
            namespace_of_file
        } else {
            namespace
        };
        let parent_fqn = crate::indexer::resolve(parent, &namespace, &imports);
        match class_methods(&parent_fqn, read, depth + 1) {
            Methods::Known(more) => methods.extend(more),
            Methods::Unknown => return Methods::Unknown,
        }
    }
    Methods::Known(methods)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_paths_follow_dots_and_skip_namespaces() {
        assert_eq!(
            view_path("orders.show").as_deref(),
            Some("resources/views/orders/show.blade.php")
        );
        assert_eq!(view_path("mail::message"), None);
        assert_eq!(view_path("bad..name"), None);
        assert_eq!(view_path("a b"), None);
    }

    #[test]
    fn livewire_components_get_a_class_and_a_view() {
        let lw = livewire("admin.user-table").unwrap();
        assert_eq!(lw.class_path, "app/Livewire/Admin/UserTable.php");
        assert!(lw.class_text.contains("namespace App\\Livewire\\Admin;"));
        assert!(lw.class_text.contains("class UserTable extends Component"));
        assert!(
            lw.class_text
                .contains("return view('livewire.admin.user-table');")
        );
        assert_eq!(
            lw.view_path,
            "resources/views/livewire/admin/user-table.blade.php"
        );
        assert_eq!(
            livewire("counter").unwrap().class_path,
            "app/Livewire/Counter.php"
        );
        assert_eq!(livewire("pkg::x"), None);
    }

    #[test]
    fn inertia_pages_follow_the_project_style() {
        let existing = vec![
            "resources/js/Pages/Home.vue".to_owned(),
            "resources/js/Pages/About.vue".to_owned(),
        ];
        assert_eq!(
            page_style(&existing, ""),
            ("resources/js/Pages".to_owned(), "vue".to_owned())
        );
        assert_eq!(
            page_style(
                &[],
                "{\"devDependencies\": {\"@inertiajs/react\": \"^2\", \"typescript\": \"^5\"}}"
            ),
            ("resources/js/Pages".to_owned(), "tsx".to_owned())
        );
        let (path, text) = inertia_page("Products/Index", "resources/js/Pages", "tsx").unwrap();
        assert_eq!(path, "resources/js/Pages/Products/Index.tsx");
        assert!(text.contains("export default function Index()"));
        assert!(
            inertia_page("Home", "resources/js/Pages", "vue")
                .unwrap()
                .1
                .contains("<template>")
        );
    }

    #[test]
    fn missing_methods_are_added_before_the_class_ends_with_the_request_import() {
        let text = "<?php\n\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Photo;\n\nclass PhotoController extends Controller\n{\n    public function index()\n    {\n        return Photo::all();\n    }\n}\n";
        let edits = add_methods(text, &["store".to_owned(), "destroy".to_owned()]).unwrap();
        let mut out = text.to_owned();
        for (s, e, t) in edits.iter().rev() {
            out.replace_range(*s..*e, t);
        }
        assert!(
            out.contains("use App\\Models\\Photo;\nuse Illuminate\\Http\\Request;\n"),
            "{out}"
        );
        assert!(out.contains("    }\n\n    public function store(Request $request)\n    {\n    }\n\n    public function destroy(string $id)\n    {\n    }\n}\n"), "{out}");
        let bare =
            "<?php\n\nnamespace App\\Http\\Controllers;\n\nclass B extends Controller\n{\n}\n";
        let edits = add_methods(bare, &["store".to_owned()]).unwrap();
        let mut out = bare.to_owned();
        for (s, e, t) in edits.iter().rev() {
            out.replace_range(*s..*e, t);
        }
        assert!(
            out.contains(
                "namespace App\\Http\\Controllers;\n\nuse Illuminate\\Http\\Request;\n\nclass B"
            ),
            "{out}"
        );
        let empty = "<?php\nnamespace App\\Http\\Controllers;\n\nclass A\n{\n}\n";
        let edits = add_methods(empty, &["index".to_owned()]).unwrap();
        let mut out = empty.to_owned();
        for (s, e, t) in edits.iter().rev() {
            out.replace_range(*s..*e, t);
        }
        assert!(
            out.ends_with("{\n    public function index()\n    {\n    }\n}\n"),
            "{out}"
        );
    }

    #[test]
    fn an_occupied_request_name_is_never_imported_twice() {
        let apply = |text: &str| {
            let mut out = text.to_owned();
            for (s, e, t) in add_methods(text, &["store".to_owned()])
                .unwrap()
                .iter()
                .rev()
            {
                out.replace_range(*s..*e, t);
            }
            out
        };
        let facade = "<?php\nnamespace App\\Http\\Controllers;\n\nuse Illuminate\\Support\\Facades\\Request;\n\nclass PhotoController extends Controller\n{\n}\n";
        let out = apply(facade);
        assert_eq!(out.matches("use ").count(), 1, "{out}");
        assert!(
            out.contains("public function store(\\Illuminate\\Http\\Request $request)"),
            "{out}"
        );
        let alias = "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Support\\Request;\nuse Illuminate\\Http\\Request as HttpRequest;\n\nclass A\n{\n}\n";
        let out = apply(alias);
        assert_eq!(out.matches("use ").count(), 2, "{out}");
        assert!(out.contains("store(HttpRequest $request)"), "{out}");
        let lower = "<?php\nnamespace App;\n\nuse Other\\request;\n\nclass A\n{\n}\n";
        assert!(apply(lower).contains("store(\\Illuminate\\Http\\Request $request)"));
        let own = "<?php\nnamespace App\\Http;\n\nclass Request\n{\n}\n";
        let out = apply(own);
        assert!(!out.contains("use "), "{out}");
        assert!(
            out.contains("store(\\Illuminate\\Http\\Request $request)"),
            "{out}"
        );
        let imported = "<?php\nnamespace App;\n\nuse Illuminate\\Http\\Request;\n\nclass A\n{\n}\n";
        let out = apply(imported);
        assert_eq!(out.matches("use ").count(), 1, "{out}");
        assert!(out.contains("store(Request $request)"), "{out}");
    }

    #[test]
    fn methods_come_from_the_class_and_its_project_parents() {
        let files: BTreeMap<&str, &str> = BTreeMap::from([
            (
                "App\\Http\\Controllers\\PhotoController",
                "<?php\nnamespace App\\Http\\Controllers;\nclass PhotoController extends Controller\n{\n    public function index() {}\n    protected function helper() {}\n}\n",
            ),
            (
                "App\\Http\\Controllers\\Controller",
                "<?php\nnamespace App\\Http\\Controllers;\nabstract class Controller\n{\n    public function shared() {}\n}\n",
            ),
            (
                "App\\Http\\Controllers\\TraitController",
                "<?php\nnamespace App\\Http\\Controllers;\nclass TraitController\n{\n    use Things;\n}\n",
            ),
            (
                "App\\Http\\Controllers\\VendorController",
                "<?php\nnamespace App\\Http\\Controllers;\nuse Vendor\\Base;\nclass VendorController extends Base\n{\n}\n",
            ),
        ]);
        let read = |fqn: &str| files.get(fqn).map(|t| ((*t).to_owned(), String::new()));
        match class_methods("App\\Http\\Controllers\\PhotoController", &read, 0) {
            Methods::Known(m) => assert_eq!(m, ["index", "shared"]),
            Methods::Unknown => panic!("known"),
        }
        assert!(matches!(
            class_methods("App\\Http\\Controllers\\TraitController", &read, 0),
            Methods::Unknown
        ));
        assert!(matches!(
            class_methods("App\\Http\\Controllers\\VendorController", &read, 0),
            Methods::Unknown
        ));
    }
}
