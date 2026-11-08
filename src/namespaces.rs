use openidea_plugin::Range;
use openidea_plugin::providers::{CompletionItem, CompletionKind};

pub fn typed_at(text: &str, cursor: usize) -> Option<usize> {
    let line_start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[line_start..cursor];
    let indent = line.len() - line.trim_start().len();
    let rest = line.trim_start().strip_prefix("namespace")?;
    let name = rest.trim_start();
    if name.len() == rest.len() {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\\')
    {
        return None;
    }
    Some(line_start + indent + "namespace".len() + (rest.len() - name.len()))
}

pub fn for_path(rel_path: &str, psr4: &[(String, String)]) -> Vec<String> {
    let rel = rel_path.replace('\\', "/");
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    let dir = format!("{dir}/");
    let mut out: Vec<String> = Vec::new();
    for (prefix, base) in psr4 {
        let Some(rest) = dir.strip_prefix(base.as_str()) else {
            continue;
        };
        let mut parts: Vec<&str> = prefix.trim_end_matches('\\').split('\\').collect();
        parts.extend(rest.split('/').filter(|p| !p.is_empty()));
        let namespace = parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("\\");
        if !namespace.is_empty() && !out.contains(&namespace) {
            out.push(namespace);
        }
    }
    out
}

pub fn items(
    namespaces: &[String],
    text: &str,
    start: usize,
    cursor: usize,
    replace: Range,
) -> Vec<CompletionItem> {
    let typed = text[start..cursor].to_ascii_lowercase();
    let closed = text[cursor..]
        .trim_start_matches([' ', '\t'])
        .starts_with(';');
    namespaces
        .iter()
        .enumerate()
        .filter(|(_, ns)| ns.to_ascii_lowercase().starts_with(&typed))
        .map(|(rank, ns)| CompletionItem {
            detail: Some("PSR-4 namespace of this folder".into()),
            insert_text: if closed { ns.clone() } else { format!("{ns};") },
            replace: Some(replace),
            sort_text: Some(format!("{rank:02}")),
            ..CompletionItem::new(ns, CompletionKind::Module)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psr4() -> Vec<(String, String)> {
        vec![
            ("Database\\Factories\\".into(), "database/factories/".into()),
            ("Tests\\".into(), "tests/".into()),
            ("App\\".into(), "app/".into()),
        ]
    }

    #[test]
    fn finds_the_namespace_being_typed() {
        let text = "<?php\n\nnamespace App\\Ht";
        assert_eq!(typed_at(text, text.len()), Some(text.find("App").unwrap()));
        let empty = "<?php\nnamespace ";
        assert_eq!(typed_at(empty, empty.len()), Some(empty.len()));
        assert_eq!(typed_at("<?php\nnamespaces", 16), None);
        assert_eq!(typed_at("<?php\n$namespace = 'x", 21), None);
    }

    #[test]
    fn maps_the_folder_through_psr4() {
        assert_eq!(
            for_path("app/Http/Controllers/Admin/UserController.php", &psr4()),
            ["App\\Http\\Controllers\\Admin"]
        );
        assert_eq!(for_path("app/User.php", &psr4()), ["App"]);
        assert_eq!(
            for_path("tests\\Feature\\Api\\OrderTest.php", &psr4()),
            ["Tests\\Feature\\Api"]
        );
        assert!(for_path("routes/web.php", &psr4()).is_empty());
    }

    #[test]
    fn completes_with_a_semicolon_unless_one_follows() {
        let ns = vec!["App\\Http\\Controllers".to_owned()];
        let text = "<?php\nnamespace App\\H";
        let start = typed_at(text, text.len()).unwrap();
        let open = items(&ns, text, start, text.len(), Range::default());
        assert_eq!(open[0].insert_text, "App\\Http\\Controllers;");
        let text = "<?php\nnamespace App\\H;";
        let closed = items(&ns, text, start, text.len() - 1, Range::default());
        assert_eq!(closed[0].insert_text, "App\\Http\\Controllers");
        assert!(items(&ns, "<?php\nnamespace Foo", 16, 19, Range::default()).is_empty());
    }
}
