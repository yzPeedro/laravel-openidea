use crate::modeldoc;
use crate::php::{Kind, tokenize};
use crate::project::{Lines, join};
use openidea_plugin::{Location, Project};

const SCALARS: [&str; 16] = [
    "int", "float", "string", "bool", "array", "mixed", "void", "null", "never", "object",
    "iterable", "callable", "static", "self", "true", "false",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub label: String,
    pub location: Option<Location>,
}

pub fn cast_class(expr: &str) -> Option<String> {
    let t = expr.trim();
    for wrapper in ["::of(", "::using("] {
        if let Some(i) = t.find(wrapper) {
            let inner = &t[i + wrapper.len()..];
            if let Some(j) = inner.find("::class") {
                return Some(inner[..j].trim().to_owned());
            }
        }
    }
    let j = t.find("::class")?;
    let name = t[..j].trim();
    (!name.is_empty() && !name.contains(['(', ' ', '\'', '"'])).then(|| name.to_owned())
}

fn short(name: &str) -> &str {
    name.trim_start_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or(name)
}

pub fn resolve_name(text: &str, name: &str) -> String {
    if let Some(full) = name.strip_prefix('\\') {
        return full.to_owned();
    }
    let tokens = tokenize(text, 0, text.len());
    let mut namespace = String::new();
    let (first, rest) = match name.split_once('\\') {
        Some((a, b)) => (a, Some(b)),
        None => (name, None),
    };
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i].kind {
            Kind::Ident(k) if k == "namespace" => {
                if let Some(Kind::Ident(ns)) = tokens.get(i + 1).map(|t| &t.kind) {
                    namespace = ns.clone();
                }
            }
            Kind::Ident(k) if k == "use" => {
                if let Some(Kind::Ident(path)) = tokens.get(i + 1).map(|t| &t.kind) {
                    let alias = match (
                        tokens.get(i + 2).map(|t| &t.kind),
                        tokens.get(i + 3).map(|t| &t.kind),
                    ) {
                        (Some(Kind::Ident(r#as)), Some(Kind::Ident(a))) if r#as == "as" => {
                            a.as_str()
                        }
                        _ => short(path),
                    };
                    if alias == first {
                        let path = path.trim_start_matches('\\');
                        return match rest {
                            Some(rest) => format!("{path}\\{rest}"),
                            None => path.to_owned(),
                        };
                    }
                }
            }
            Kind::Ident(k) if matches!(k.as_str(), "class" | "enum" | "interface" | "trait") => {
                break;
            }
            _ => {}
        }
        i += 1;
    }
    if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}\\{name}")
    }
}

pub fn psr4(composer: &str) -> Vec<(String, String)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(composer) else {
        return vec![("App\\".into(), "app/".into())];
    };
    let mut out = Vec::new();
    for section in ["autoload", "autoload-dev"] {
        if let Some(map) = json[section]["psr-4"].as_object() {
            for (prefix, dir) in map {
                let dirs: Vec<String> = match dir {
                    serde_json::Value::String(d) => vec![d.clone()],
                    serde_json::Value::Array(a) => a
                        .iter()
                        .filter_map(|d| d.as_str().map(str::to_owned))
                        .collect(),
                    _ => Vec::new(),
                };
                for d in dirs {
                    let d = d.trim_end_matches('/');
                    out.push((
                        prefix.clone(),
                        if d.is_empty() {
                            String::new()
                        } else {
                            format!("{d}/")
                        },
                    ));
                }
            }
        }
    }
    if out.is_empty() {
        out.push(("App\\".into(), "app/".into()));
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
    out
}

fn file_of(project: &Project, map: &[(String, String)], fqn: &str) -> Option<(String, String)> {
    for (prefix, dir) in map {
        if let Some(rest) = fqn.strip_prefix(prefix.as_str()) {
            let path = format!("{dir}{}.php", rest.replace('\\', "/"));
            if let Ok(text) = project.read_file(&path) {
                return Some((path, text));
            }
        }
    }
    None
}

fn declaration(text: &str, name: &str) -> Option<(String, usize, usize)> {
    let tokens = tokenize(text, 0, text.len());
    tokens
        .windows(2)
        .find_map(|w| match (&w[0].kind, &w[1].kind) {
            (Kind::Ident(k), Kind::Ident(n))
                if n == name && matches!(k.as_str(), "class" | "enum" | "interface" | "trait") =>
            {
                Some((k.clone(), w[1].start, w[1].end))
            }
            _ => None,
        })
}

pub fn get_return_type(text: &str) -> Option<String> {
    let at = text.find("function get(")?;
    let tokens = tokenize(text, at, text.len().min(at + 4000));
    let open = tokens.iter().position(|t| t.kind == Kind::Open('('))?;
    let mut depth = 0;
    let mut close = None;
    for (k, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            Kind::Open('(') => depth += 1,
            Kind::Close(')') => {
                depth -= 1;
                if depth == 0 {
                    close = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    if tokens.get(close + 1).map(|t| &t.kind) != Some(&Kind::Colon) {
        return None;
    }
    let start = tokens.get(close + 2)?.start;
    let end = tokens[close + 2..]
        .iter()
        .find(|t| matches!(t.kind, Kind::Open('{') | Kind::Semicolon))
        .map_or(text.len(), |t| t.start);
    let ty = text[start..end].trim().trim_start_matches('?');
    let pick = ty
        .split('|')
        .map(str::trim)
        .find(|t| !t.eq_ignore_ascii_case("null"))
        .unwrap_or(ty);
    (!pick.is_empty()).then(|| pick.to_owned())
}

fn location(root: &str, path: &str, text: &str, start: usize, end: usize) -> Location {
    let lines = Lines::new(text);
    Location {
        path: join(root, path),
        range: lines.range(text, start, end),
    }
}

pub fn resolve(project: &Project, root: &str, model_text: &str, column: &str) -> Option<Target> {
    let expr = modeldoc::casts(model_text)
        .into_iter()
        .find(|(c, _)| c == column)
        .map(|(_, e)| e)?;
    let Some(class) = cast_class(&expr) else {
        let label = modeldoc::cast_type(&expr)?;
        let label = label.split('|').map(short).collect::<Vec<_>>().join("|");
        return Some(Target {
            label,
            location: None,
        });
    };
    let composer = project.read_file("composer.json").unwrap_or_default();
    let map = psr4(&composer);
    let fqn = resolve_name(model_text, &class);
    let name = short(&fqn).to_owned();
    let Some((path, text)) = file_of(project, &map, &fqn) else {
        return Some(Target {
            label: name,
            location: None,
        });
    };
    let Some((keyword, s, e)) = declaration(&text, &name) else {
        return Some(Target {
            label: name,
            location: None,
        });
    };
    let own = location(root, &path, &text, s, e);
    if keyword == "enum" {
        return Some(Target {
            label: name,
            location: Some(own),
        });
    }
    let Some(result) = get_return_type(&text) else {
        return Some(Target {
            label: name,
            location: Some(own),
        });
    };
    if SCALARS.contains(&result.to_ascii_lowercase().as_str()) {
        let label = if matches!(
            result.to_ascii_lowercase().as_str(),
            "mixed" | "static" | "self"
        ) {
            name
        } else {
            result
        };
        return Some(Target {
            label,
            location: Some(own),
        });
    }
    let result_fqn = resolve_name(&text, &result);
    let result_name = short(&result_fqn).to_owned();
    let found = file_of(project, &map, &result_fqn).and_then(|(p, t)| {
        let (_, s, e) = declaration(&t, &result_name)?;
        Some(location(root, &p, &t, s, e))
    });
    Some(Target {
        label: result_name,
        location: Some(found.unwrap_or(own)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_class_of_a_cast() {
        assert_eq!(
            cast_class("OrderStatus::class").as_deref(),
            Some("OrderStatus")
        );
        assert_eq!(
            cast_class("\\App\\Casts\\Money::class").as_deref(),
            Some("\\App\\Casts\\Money")
        );
        assert_eq!(
            cast_class("AsEnumCollection::of(Tag::class)").as_deref(),
            Some("Tag")
        );
        assert_eq!(
            cast_class("AsCollection::using(Items::class)").as_deref(),
            Some("Items")
        );
        assert_eq!(
            cast_class("Address::class.':nullable'").as_deref(),
            Some("Address")
        );
        assert_eq!(cast_class("'datetime'"), None);
    }

    #[test]
    fn resolves_names_like_php() {
        let text = "<?php\nnamespace App\\Models;\n\nuse App\\Casts\\Money;\nuse App\\Enums\\Status as State;\nuse App\\Support;\n\nclass Order extends Model {}\n";
        assert_eq!(resolve_name(text, "Money"), "App\\Casts\\Money");
        assert_eq!(resolve_name(text, "State"), "App\\Enums\\Status");
        assert_eq!(resolve_name(text, "Support\\Price"), "App\\Support\\Price");
        assert_eq!(resolve_name(text, "Review"), "App\\Models\\Review");
        assert_eq!(resolve_name(text, "\\Foo\\Bar"), "Foo\\Bar");
    }

    #[test]
    fn reads_the_type_a_cast_returns() {
        let text = "<?php\nclass Money implements CastsAttributes\n{\n    public function get(Model $model, string $key, mixed $value, array $attributes): ?Price\n    {\n        return new Price($value);\n    }\n}\n";
        assert_eq!(get_return_type(text).as_deref(), Some("Price"));
        assert_eq!(
            get_return_type("<?php function get($a, $b): float|null {}").as_deref(),
            Some("float")
        );
        assert_eq!(
            get_return_type("<?php function get($a) { return 1; }"),
            None
        );
    }

    #[test]
    fn maps_namespaces_to_folders() {
        let composer = r#"{"autoload": {"psr-4": {"App\\": "app/", "Domain\\": ["src/Domain", "lib"]}}, "autoload-dev": {"psr-4": {"Tests\\": "tests/"}}}"#;
        let map = psr4(composer);
        assert!(map.contains(&("App\\".into(), "app/".into())));
        assert!(map.contains(&("Domain\\".into(), "src/Domain/".into())));
        assert!(map.contains(&("Tests\\".into(), "tests/".into())));
    }
}
