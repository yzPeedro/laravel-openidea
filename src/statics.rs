use crate::php::{Kind, tokenize};
use openidea_plugin::Range;
use openidea_plugin::providers::{CompletionItem, CompletionKind};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Static,
    CastValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub fqn: String,
    pub detail: String,
    pub cast: bool,
    pub statically: bool,
}

const NOT_AN_EXPRESSION: [&str; 9] = [
    "use",
    "instanceof",
    "namespace",
    "catch",
    "insteadof",
    "as",
    "public",
    "protected",
    "private",
];

fn short(fqn: &str) -> &str {
    fqn.rsplit('\\').next().unwrap_or(fqn)
}

fn previous_word(text: &str) -> &str {
    let trimmed = text.trim_end();
    let start = trimmed
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(0, |i| i + 1);
    &trimmed[start..]
}

fn in_signature(before: &str) -> bool {
    let mut depth = 0i32;
    for (i, c) in before.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' if depth == 0 => {
                let head = before[..i].trim_end();
                let head = head
                    .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
                    .trim_end();
                return head.ends_with("function") || head.ends_with("fn");
            }
            '(' => depth -= 1,
            ';' | '{' | '}' => return false,
            _ => {}
        }
    }
    false
}

fn in_casts(before: &str) -> bool {
    let anchor = before
        .rfind("$casts")
        .into_iter()
        .chain(before.rfind("function casts"))
        .max();
    let Some(anchor) = anchor else {
        return false;
    };
    let mut depth = 0i32;
    for c in before[anchor..].chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
    }
    depth > 0
}

pub fn slot_at(text: &str, cursor: usize) -> Option<(Slot, usize)> {
    let start = crate::snippets::word_at(text, cursor)?;
    if !text[start..cursor].starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let before = &text[..start];
    let trimmed = before.trim_end();
    if trimmed.ends_with("=>") && in_casts(trimmed) {
        return Some((Slot::CastValue, start));
    }
    if crate::snippets::place_at(text, cursor) != Some(crate::snippets::Place::Code) {
        return None;
    }
    if NOT_AN_EXPRESSION.contains(&previous_word(before)) {
        return None;
    }
    let prev = trimmed.chars().next_back();
    if matches!(prev, Some('|' | '&' | '?')) || trimmed.ends_with("#[") || trimmed.ends_with("):") {
        return None;
    }
    if matches!(prev, Some('(' | ',')) && in_signature(trimmed) {
        return None;
    }
    Some((Slot::Static, start))
}

pub fn config_aliases(config: &str) -> Vec<(String, String)> {
    let tokens = tokenize(config, 0, config.len());
    let Some(at) = tokens
        .iter()
        .position(|t| matches!(&t.kind, Kind::Str { value, .. } if value == "aliases"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut i = at + 1;
    while i < tokens.len() {
        match &tokens[i].kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => {
                depth -= 1;
                if depth <= 0 {
                    break;
                }
            }
            Kind::Str { value, .. }
                if depth == 1
                    && tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == Kind::DoubleArrow) =>
            {
                if let Some(Kind::Ident(class)) = tokens.get(i + 2).map(|t| &t.kind)
                    && tokens
                        .get(i + 3)
                        .is_some_and(|t| t.kind == Kind::DoubleColon)
                {
                    out.push((value.clone(), class.trim_start_matches('\\').to_owned()));
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn namespace_of(text: &str) -> &str {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("namespace "))
        .map_or("", |ns| ns.trim_end_matches([';', ' ', '{']).trim())
}

pub fn items(
    slot: Slot,
    text: &str,
    typed: &str,
    replace: Range,
    candidates: &[Candidate],
) -> Vec<CompletionItem> {
    let typed = typed.to_lowercase();
    let namespace = namespace_of(text);
    let mut seen = BTreeSet::new();
    candidates
        .iter()
        .filter(|c| match slot {
            Slot::Static => c.statically,
            Slot::CastValue => c.cast,
        })
        .filter(|c| short(&c.fqn).to_lowercase().contains(&typed))
        .filter(|c| seen.insert(short(&c.fqn).to_owned()))
        .map(|c| {
            let name = short(&c.fqn);
            let suffix = match slot {
                Slot::Static => "::",
                Slot::CastValue => "::class",
            };
            let label = format!("{name}{suffix}");
            let rank = if name.to_lowercase().starts_with(&typed) {
                '0'
            } else {
                '1'
            };
            let own = c.fqn.rsplit_once('\\').map_or("", |(ns, _)| ns);
            let additional_edits = if own == namespace {
                Vec::new()
            } else {
                crate::snippets::import_edits(text, &[c.fqn.as_str()])
            };
            CompletionItem {
                detail: Some(c.detail.clone()),
                replace: Some(replace),
                sort_text: Some(format!("{rank}{name}")),
                filter_text: Some(name.to_owned()),
                additional_edits,
                ..CompletionItem::new(&label, CompletionKind::Class)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(text: &str) -> Option<Slot> {
        let cursor = text.find('|').unwrap();
        let text = text.replace('|', "");
        slot_at(&text, cursor).map(|(s, _)| s)
    }

    #[test]
    fn finds_the_places_where_a_class_is_used_statically() {
        assert_eq!(
            slot("<?php\nfunction a() {\n    Cac|\n}\n"),
            Some(Slot::Static)
        );
        assert_eq!(
            slot("<?php\nfunction a() {\n    $x = Ord|\n}\n"),
            Some(Slot::Static)
        );
        assert_eq!(
            slot("<?php\nfunction a() {\n    f(1, Ord|\n}\n"),
            Some(Slot::Static)
        );
        assert_eq!(
            slot("<?php\nfunction a() {\n    return new Ord|\n}\n"),
            None
        );
        assert_eq!(slot("<?php\nfunction a(Ord|\n"), None);
        assert_eq!(slot("<?php\nfunction a(int $a, Ord|\n"), None);
        assert_eq!(slot("<?php\nfn (Ord|\n"), None);
        assert_eq!(slot("<?php\nfunction a(): Ord|\n"), None);
        assert_eq!(
            slot("<?php\nfunction a() {\n    $x instanceof Ord|\n}\n"),
            None
        );
        assert_eq!(slot("<?php\nuse Ord|\n"), None);
        assert_eq!(slot("<?php\nfunction a() {\n    $x->Ord|\n}\n"), None);
        assert_eq!(slot("<?php\nfunction a() {\n    'Ord|'\n}\n"), None);
    }

    #[test]
    fn cast_values_get_the_class_constant() {
        assert_eq!(
            slot(
                "<?php\nclass A extends Model\n{\n    protected $casts = [\n        'status' => Ord|\n    ];\n}\n"
            ),
            Some(Slot::CastValue)
        );
        assert_eq!(
            slot(
                "<?php\nclass A extends Model\n{\n    protected function casts(): array\n    {\n        return ['status' => Ord|];\n    }\n}\n"
            ),
            Some(Slot::CastValue)
        );
    }

    #[test]
    fn reads_the_aliases_of_older_laravel_versions() {
        let config = "<?php\nreturn [\n    'name' => 'x',\n    'aliases' => [\n        'App' => Illuminate\\Support\\Facades\\App::class,\n        'Arr' => Illuminate\\Support\\Arr::class,\n    ],\n];\n";
        assert_eq!(
            config_aliases(config),
            [
                (
                    "App".to_owned(),
                    "Illuminate\\Support\\Facades\\App".to_owned()
                ),
                ("Arr".to_owned(), "Illuminate\\Support\\Arr".to_owned()),
            ]
        );
    }

    #[test]
    fn inserts_the_double_colon_and_the_import() {
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nclass A\n{\n    public function b() { Cac }\n}\n";
        let candidates = [
            Candidate {
                fqn: "Illuminate\\Support\\Facades\\Cache".into(),
                detail: "Laravel facade".into(),
                cast: false,
                statically: true,
            },
            Candidate {
                fqn: "App\\Enums\\OrderStatus".into(),
                detail: "enum".into(),
                cast: true,
                statically: true,
            },
        ];
        let found = items(Slot::Static, text, "Cac", Range::default(), &candidates);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "Cache::");
        assert_eq!(found[0].insert_text, "Cache::");
        assert!(
            found[0].additional_edits[0]
                .text
                .contains("use Illuminate\\Support\\Facades\\Cache;")
        );
        let cast = items(Slot::CastValue, text, "ord", Range::default(), &candidates);
        assert_eq!(cast[0].insert_text, "OrderStatus::class");
    }
}
