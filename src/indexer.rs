use crate::Laravel;
use crate::index::RefKind;
use crate::php::{Kind, Token, tokenize};
use crate::project::{Lines, rel};
use crate::{casts, eloquent, resources, usages};
use openidea_plugin::indexing::{Anchor, Document, DocumentIndexer, Edges};
use std::collections::BTreeMap;

pub const KEY: &str = "laravel-usages#";

const DECLARES: [&str; 5] = ["class", "interface", "trait", "enum", "function"];

pub fn class_key(fqn: &str) -> String {
    format!("{KEY}{}", fqn.trim_start_matches('\\'))
}

pub fn scope_key(model: &str, scope: &str) -> String {
    format!("{KEY}scope:{model}.{scope}")
}

fn ident(t: Option<&Token>) -> Option<&str> {
    match &t?.kind {
        Kind::Ident(s) => Some(s),
        _ => None,
    }
}

pub fn imports(text: &str) -> (String, BTreeMap<String, String>) {
    let toks = tokenize(text, 0, text.len());
    let mut namespace = String::new();
    let mut map = BTreeMap::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < toks.len() {
        match &toks[i].kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => depth = depth.saturating_sub(1),
            Kind::Ident(k) if depth == 0 && k == "namespace" => {
                if let Some(ns) = ident(toks.get(i + 1)) {
                    namespace = ns.trim_start_matches('\\').to_owned();
                }
            }
            Kind::Ident(k) if depth == 0 && k == "use" => {
                let end = (i..toks.len())
                    .find(|&j| toks[j].kind == Kind::Semicolon)
                    .unwrap_or(toks.len());
                read_use(&toks[i + 1..end], &mut map);
                i = end;
            }
            _ => {}
        }
        i += 1;
    }
    (namespace, map)
}

fn read_use(toks: &[Token], map: &mut BTreeMap<String, String>) {
    let skip = matches!(ident(toks.first()), Some("function" | "const"));
    let toks = if skip { &toks[1..] } else { toks };
    let Some(head) = ident(toks.first()) else {
        return;
    };
    let head = head.trim_start_matches('\\').trim_end_matches('\\');
    if toks.get(1).is_some_and(|t| t.kind == Kind::Open('{')) {
        let mut j = 2;
        while j < toks.len() {
            if let Some(part) = ident(toks.get(j)) {
                let full = format!("{head}\\{}", part.trim_start_matches('\\'));
                let alias = if ident(toks.get(j + 1)) == Some("as") {
                    j += 2;
                    ident(toks.get(j)).unwrap_or_default().to_owned()
                } else {
                    full.rsplit('\\').next().unwrap_or(&full).to_owned()
                };
                if !skip {
                    map.insert(alias, full);
                }
            }
            j += 1;
        }
        return;
    }
    let mut j = 0;
    while j < toks.len() {
        if let Some(path) = ident(toks.get(j)) {
            let full = path.trim_start_matches('\\').to_owned();
            let alias = if ident(toks.get(j + 1)) == Some("as") {
                j += 2;
                ident(toks.get(j)).unwrap_or_default().to_owned()
            } else {
                full.rsplit('\\').next().unwrap_or(&full).to_owned()
            };
            if !skip {
                map.insert(alias, full);
            }
        }
        j += 1;
    }
}

pub fn resolve(name: &str, namespace: &str, imports: &BTreeMap<String, String>) -> String {
    if let Some(full) = name.strip_prefix('\\') {
        return full.to_owned();
    }
    let (first, rest) = match name.split_once('\\') {
        Some((a, b)) => (a, Some(b)),
        None => (name, None),
    };
    if let Some(full) = imports.get(first) {
        return match rest {
            Some(rest) => format!("{full}\\{rest}"),
            None => full.clone(),
        };
    }
    if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}\\{name}")
    }
}

fn constant_like(name: &str) -> bool {
    name.len() > 1
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub fn class_usages(text: &str, cleaned: &str, prefixes: &[String]) -> Vec<(String, usize, usize)> {
    let (namespace, imported) = imports(text);
    let toks = tokenize(cleaned, 0, cleaned.len());
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let Kind::Ident(name) = &t.kind else {
            continue;
        };
        let short = name.rsplit('\\').next().unwrap_or(name);
        if !short.starts_with(|c: char| c.is_ascii_uppercase()) || constant_like(short) {
            continue;
        }
        let previous = i.checked_sub(1).map(|p| &toks[p]);
        if previous.is_some_and(|p| matches!(p.kind, Kind::ObjectOp | Kind::DoubleColon))
            || ident(previous).is_some_and(|p| DECLARES.contains(&p) || p == "namespace")
        {
            continue;
        }
        let called = toks.get(i + 1).is_some_and(|n| n.kind == Kind::Open('('));
        if called && ident(previous) != Some("new") {
            continue;
        }
        let fqn = resolve(name, &namespace, &imported);
        if !prefixes.iter().any(|p| fqn.starts_with(p.as_str())) {
            continue;
        }
        out.push((fqn, t.end - short.len(), t.end));
    }
    out
}

fn word_in_line(text: &str, line: u32, word: &str) -> Option<usize> {
    let start: usize = text
        .split_inclusive('\n')
        .take(line.saturating_sub(1) as usize)
        .map(str::len)
        .sum();
    let line_text = text[start..].lines().next()?;
    let at = crate::modelmap::first_offset(line_text, word)?;
    Some(start + at)
}

impl Laravel {
    pub(crate) fn usage_edges(&mut self, path: &str, text: &str) -> Edges {
        let mut edges = Edges::default();
        let Some((project, root)) = self.laravel_project(path) else {
            return edges;
        };
        let own = rel(&root, path).replace('\\', "/");
        if !usages::indexed(&own) {
            return edges;
        }
        let blade = own.ends_with(".blade.php");
        let lines = Lines::new(text);
        let anchor = |key: String, start: usize, end: usize| Anchor {
            key,
            range: lines.range(text, start, end),
        };
        let composer = project.read_file("composer.json").unwrap_or_default();
        let prefixes: Vec<String> = casts::psr4(&composer)
            .into_iter()
            .map(|(namespace, _)| namespace)
            .filter(|n| !n.is_empty())
            .collect();
        let cleaned = usages::clean(text, blade);
        for (fqn, start, end) in class_usages(text, &cleaned, &prefixes) {
            edges.definitions.push(anchor(class_key(&fqn), start, end));
        }
        let scopes: Vec<(String, String)> = self
            .cache
            .get(RefKind::Scope, &project, &root)
            .iter()
            .filter_map(|e| {
                e.key
                    .split_once('.')
                    .map(|(m, s)| (m.to_owned(), s.to_owned()))
            })
            .collect();
        if !blade {
            for (model, scope) in &scopes {
                for (start, end) in eloquent::scope_calls(&cleaned, model, scope) {
                    edges
                        .definitions
                        .push(anchor(scope_key(model, scope), start, end));
                }
            }
        }
        if blade || !(own.starts_with("app/") || own.starts_with("database/")) {
            return edges;
        }
        let model = self.model_of_file(&project, &root, path);
        for symbol in resources::scan_text(&own, text).symbols {
            let fqn = if symbol.namespace.is_empty() {
                symbol.name.clone()
            } else {
                format!("{}\\{}", symbol.namespace, symbol.name)
            };
            if let Some(at) = word_in_line(text, symbol.line, &symbol.name) {
                edges
                    .markers
                    .push(anchor(class_key(&fqn), at, at + symbol.name.len()));
            }
            if model.as_deref() != Some(symbol.name.as_str()) {
                continue;
            }
            for method in &symbol.methods {
                let Some(scope) = crate::scope_name(method) else {
                    continue;
                };
                if let Some(at) = word_in_line(text, method.line, &method.name) {
                    edges.markers.push(anchor(
                        scope_key(&symbol.name, &scope),
                        at,
                        at + method.name.len(),
                    ));
                }
            }
        }
        edges
    }
}

impl DocumentIndexer for Laravel {
    fn index(&mut self, doc: &Document) -> Edges {
        let path = crate::project::plain_path(&doc.path).into_owned();
        self.usage_edges(&path, &doc.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_namespace_imports_aliases_and_groups() {
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Order;\nuse App\\Models\\{Item, User as Customer};\nuse function strlen;\nuse Illuminate\\Support\\Facades\\Cache as Store;\n\nclass A {}\n";
        let (ns, map) = imports(text);
        assert_eq!(ns, "App\\Http\\Controllers");
        assert_eq!(map["Order"], "App\\Models\\Order");
        assert_eq!(map["Item"], "App\\Models\\Item");
        assert_eq!(map["Customer"], "App\\Models\\User");
        assert_eq!(map["Store"], "Illuminate\\Support\\Facades\\Cache");
        assert!(!map.contains_key("strlen"));
        assert_eq!(resolve("Order", &ns, &map), "App\\Models\\Order");
        assert_eq!(resolve("Customer", &ns, &map), "App\\Models\\User");
        assert_eq!(resolve("\\App\\Jobs\\Ship", &ns, &map), "App\\Jobs\\Ship");
        assert_eq!(resolve("Local", &ns, &map), "App\\Http\\Controllers\\Local");
    }

    #[test]
    fn finds_class_usages_in_code_only() {
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Order;\nuse Illuminate\\Support\\Facades\\Cache;\n\n// Order::all();\nclass OrderController extends Controller\n{\n    public function show(Order $order): OrderResource\n    {\n        $x = 'App\\Models\\Order';\n        Cache::get('k');\n        $order->Order;\n        new \\App\\Jobs\\Ship($order);\n        return Order::query()->first() instanceof Order ? 1 : PHP_EOL;\n    }\n}\n";
        let cleaned = usages::clean(text, false);
        let found = class_usages(text, &cleaned, &["App\\".to_owned()]);
        let names: Vec<(&str, &str)> = found
            .iter()
            .map(|(fqn, s, e)| (fqn.as_str(), &text[*s..*e]))
            .collect();
        assert_eq!(
            names,
            [
                ("App\\Http\\Controllers\\Controller", "Controller"),
                ("App\\Models\\Order", "Order"),
                ("App\\Http\\Controllers\\OrderResource", "OrderResource"),
                ("App\\Jobs\\Ship", "Ship"),
                ("App\\Models\\Order", "Order"),
                ("App\\Models\\Order", "Order"),
            ]
        );
    }
}
