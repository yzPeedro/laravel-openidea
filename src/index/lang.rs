use super::Entry;
use crate::php::{Kind, tokenize};
use crate::project::{Lines, find, rel};
use openidea_plugin::{Location, Project};
use std::collections::BTreeMap;

pub fn index(project: &Project, root: &str, locale: &str) -> Vec<Entry> {
    let mut by_key: BTreeMap<String, Vec<(bool, Entry)>> = BTreeMap::new();
    for glob in [
        "lang/**/*.php",
        "resources/lang/**/*.php",
        "lang/*.json",
        "resources/lang/*.json",
    ] {
        for path in find(project, root, glob, 5000) {
            let relative = rel(root, &path).replace('\\', "/");
            let inner = relative.strip_prefix("resources/").unwrap_or(&relative);
            let Some(inner) = inner.strip_prefix("lang/") else {
                continue;
            };
            let Ok(text) = crate::project::read_listed(project, &path) else {
                continue;
            };
            let entries = if let Some(file_locale) = inner.strip_suffix(".json") {
                json(file_locale, &path, &text)
            } else {
                let Some((file_locale, file)) = inner.split_once('/') else {
                    continue;
                };
                if file_locale == "vendor" {
                    continue;
                }
                let prefix = file.trim_end_matches(".php").replace('/', ".");
                php(file_locale, &prefix, &path, &text)
            };
            for (entry_locale, entry) in entries {
                by_key
                    .entry(entry.key.clone())
                    .or_default()
                    .push((entry_locale == locale, entry));
            }
        }
    }
    by_key
        .into_values()
        .flat_map(|mut v| {
            v.sort_by_key(|(preferred, _)| !preferred);
            v.into_iter().map(|(_, e)| e)
        })
        .collect()
}

fn php(locale: &str, prefix: &str, path: &str, text: &str) -> Vec<(String, Entry)> {
    super::config::entries(prefix.to_owned(), locale, path, text)
        .into_iter()
        .skip(1)
        .map(|mut e| {
            e.detail = Some(match e.detail {
                Some(v) if v != locale => format!("{locale}: {v}"),
                _ => locale.to_owned(),
            });
            (locale.to_owned(), e)
        })
        .collect()
}

fn json(locale: &str, path: &str, text: &str) -> Vec<(String, Entry)> {
    let lines = Lines::new(text);
    let tokens = tokenize(text, 0, text.len());
    let mut depth = 0;
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        match &t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth -= 1,
            Kind::Str { value, .. }
                if depth == 1 && tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Colon) =>
            {
                let translated = match tokens.get(i + 2).map(|n| &n.kind) {
                    Some(Kind::Str { value, .. }) => format!("{locale}: '{value}'"),
                    _ => locale.to_owned(),
                };
                out.push((
                    locale.to_owned(),
                    Entry {
                        key: value.clone(),
                        detail: Some(translated),
                        location: Location {
                            path: path.to_owned(),
                            range: lines.range(text, t.start, t.end),
                        },
                    },
                ));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_php_and_json_translations_preferring_the_locale() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file(
            "lang/en/auth.php",
            "<?php\nreturn ['failed' => 'These credentials do not match.'];\n",
        );
        host.add_file(
            "lang/pt_BR/auth.php",
            "<?php\nreturn ['failed' => 'Credenciais inválidas.'];\n",
        );
        host.add_file("lang/pt_BR.json", "{\n  \"Welcome\": \"Bem-vindo\"\n}\n");
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let entries = index(&project, "/project", "pt_BR");
        let keys: Vec<_> = entries
            .iter()
            .map(|e| (e.key.as_str(), e.detail.as_deref().unwrap_or("")))
            .collect();
        assert_eq!(
            keys,
            [
                ("Welcome", "pt_BR: 'Bem-vindo'"),
                ("auth.failed", "pt_BR: 'Credenciais inválidas.'"),
                ("auth.failed", "en: 'These credentials do not match.'"),
            ]
        );
    }
}
