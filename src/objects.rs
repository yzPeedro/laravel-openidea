use crate::casts::{file_of, psr4, resolve_name};
use crate::php::{Kind, Token, tokenize};
use openidea_plugin::Project;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Members {
    pub kind: String,
    pub name: String,
    pub backed: Option<String>,
    pub cases: Vec<String>,
    pub values: Vec<(String, String)>,
    pub methods: Vec<(String, String, bool)>,
    pub properties: Vec<(String, String)>,
    pub traits: Vec<String>,
    pub extends: Option<String>,
    pub hidden: Vec<(String, String)>,
    pub offsets: Vec<(String, usize)>,
}

fn ident(tokens: &[Token], i: usize) -> Option<&str> {
    match &tokens.get(i)?.kind {
        Kind::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn read(text: &str) -> Option<Members> {
    let tokens = tokenize(text, 0, text.len());
    let at = tokens.iter().enumerate().position(|(i, _)| {
        ident(&tokens, i).is_some_and(|k| matches!(k, "class" | "enum" | "trait" | "interface"))
            && ident(&tokens, i + 1).is_some()
            && !matches!(
                tokens.get(i.wrapping_sub(1)).map(|t| &t.kind),
                Some(Kind::DoubleColon)
            )
    })?;
    let mut m = Members {
        kind: ident(&tokens, at)?.to_owned(),
        name: ident(&tokens, at + 1)?.to_owned(),
        ..Members::default()
    };
    let open = (at..tokens.len()).find(|&i| tokens[i].kind == Kind::Open('{'))?;
    m.extends = (at + 2..open)
        .find(|&i| ident(&tokens, i).is_some_and(|w| w.eq_ignore_ascii_case("extends")))
        .and_then(|i| ident(&tokens, i + 1))
        .map(str::to_owned);
    if m.kind == "enum" && tokens.get(at + 2).map(|t| &t.kind) == Some(&Kind::Colon) {
        m.backed = ident(&tokens, at + 3).map(str::to_owned);
    }
    let mut depth = 0usize;
    let mut i = open;
    while i < tokens.len() {
        match &tokens[i].kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        if depth != 1 {
            i += 1;
            continue;
        }
        match ident(&tokens, i) {
            Some("case") if m.kind == "enum" => {
                if let Some(name) = ident(&tokens, i + 1) {
                    m.cases.push(name.to_owned());
                    if tokens.get(i + 2).is_some_and(|t| t.kind == Kind::Assign) {
                        let value = match tokens.get(i + 3).map(|t| &t.kind) {
                            Some(Kind::Str { value, .. }) => Some(value.clone()),
                            Some(Kind::Num(n)) => Some(n.clone()),
                            _ => None,
                        };
                        if let Some(value) = value {
                            m.values.push((name.to_owned(), value));
                        }
                    }
                }
            }
            Some("use") => {
                let mut j = i + 1;
                while let Some(name) = ident(&tokens, j) {
                    m.traits.push(name.to_owned());
                    if tokens.get(j + 1).map(|t| &t.kind) != Some(&Kind::Comma) {
                        break;
                    }
                    j += 2;
                }
            }
            Some("function") => {
                if let Some(name) = ident(&tokens, i + 1) {
                    let mut k = i;
                    let mut public = true;
                    let mut is_static = false;
                    while k > 0 {
                        k -= 1;
                        match ident(&tokens, k) {
                            Some("private" | "protected") => public = false,
                            Some("static") => is_static = true,
                            Some("public" | "final" | "abstract") => {}
                            _ => break,
                        }
                    }
                    let sig_start = tokens[i + 1].end;
                    let mut j = i + 2;
                    let mut paren = 0i32;
                    while j < tokens.len() {
                        match tokens[j].kind {
                            Kind::Open('(') => paren += 1,
                            Kind::Close(')') => paren -= 1,
                            Kind::Open('{') | Kind::Semicolon if paren == 0 => break,
                            _ => {}
                        }
                        j += 1;
                    }
                    let sig_end = tokens.get(j).map_or(text.len(), |t| t.start);
                    m.offsets.push((name.to_owned(), tokens[i + 1].start));
                    if public && name != "__construct" && !name.starts_with("__") {
                        m.methods.push((
                            name.to_owned(),
                            collapse(&text[sig_start..sig_end]),
                            is_static,
                        ));
                    } else if !public {
                        m.hidden
                            .push((name.to_owned(), collapse(&text[sig_start..sig_end])));
                    }
                    if name == "__construct" {
                        let mut p = i + 2;
                        while p < j {
                            if ident(&tokens, p) == Some("public") {
                                let mut q = p + 1;
                                while q < j {
                                    if let Kind::Var(v) = &tokens[q].kind {
                                        let ty = collapse(&text[tokens[p].end..tokens[q].start])
                                            .replace("readonly", "")
                                            .trim()
                                            .to_owned();
                                        let name = v.trim_start_matches('$').to_owned();
                                        if !m.properties.iter().any(|(n, _)| n == &name) {
                                            m.properties.push((name, ty));
                                        }
                                        break;
                                    }
                                    q += 1;
                                }
                            }
                            p += 1;
                        }
                    }
                }
            }
            Some("public") => {
                let mut j = i + 1;
                while let Some(word) = ident(&tokens, j) {
                    if matches!(word, "function" | "const" | "static") {
                        break;
                    }
                    j += 1;
                }
                if let Some(Kind::Var(v)) = tokens.get(j).map(|t| &t.kind)
                    && ident(&tokens, i + 1) != Some("function")
                {
                    let ty = collapse(&text[tokens[i].end..tokens[j].start])
                        .replace("readonly", "")
                        .trim()
                        .to_owned();
                    let name = v.trim_start_matches('$').to_owned();
                    if !m.properties.iter().any(|(n, _)| n == &name) {
                        m.properties.push((name, ty));
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    Some(m)
}

pub fn with_traits(project: &Project, path_text: (&str, &str), depth: u8) -> Option<Members> {
    let (_, text) = path_text;
    let mut members = read(text)?;
    if depth > 3 {
        return Some(members);
    }
    let composer = project.read_file("composer.json").unwrap_or_default();
    let map = psr4(&composer);
    for name in members.traits.clone() {
        let fqn = resolve_name(text, &name);
        let Some((path, trait_text)) = file_of(project, &map, &fqn) else {
            continue;
        };
        let Some(used) = with_traits(project, (&path, &trait_text), depth + 1) else {
            continue;
        };
        for method in used.methods {
            if !members.methods.iter().any(|(n, _, _)| n == &method.0) {
                members.methods.push(method);
            }
        }
        for property in used.properties {
            if !members.properties.iter().any(|(n, _)| n == &property.0) {
                members.properties.push(property);
            }
        }
    }
    Some(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_enum_cases_methods_and_traits() {
        let text = "<?php\nnamespace App\\Enums;\n\nuse App\\Concerns\\HasLabel;\n\nenum OrderStatus: string implements HasColor\n{\n    use HasLabel, Comparable;\n\n    case APPROVED = 'approved';\n    case PENDING = 'pending';\n\n    public function color(): string\n    {\n        return match ($this) { self::APPROVED => 'green', self::PENDING => 'gray' };\n    }\n\n    public static function default(): self\n    {\n        return self::PENDING;\n    }\n\n    private function secret(): void {}\n}\n";
        let m = read(text).unwrap();
        assert_eq!((m.kind.as_str(), m.name.as_str()), ("enum", "OrderStatus"));
        assert_eq!(m.backed.as_deref(), Some("string"));
        assert_eq!(m.cases, ["APPROVED", "PENDING"]);
        assert_eq!(
            m.values,
            [
                ("APPROVED".to_owned(), "approved".to_owned()),
                ("PENDING".to_owned(), "pending".to_owned())
            ]
        );
        assert_eq!(m.traits, ["HasLabel", "Comparable"]);
        assert_eq!(
            m.methods,
            [
                ("color".to_owned(), "(): string".to_owned(), false),
                ("default".to_owned(), "(): self".to_owned(), true)
            ]
        );
    }

    #[test]
    fn reads_public_and_promoted_properties() {
        let text = "<?php\nfinal class Money\n{\n    public int $cents = 0;\n    protected string $hidden;\n\n    public function __construct(public readonly string $currency, private int $raw = 0) {}\n\n    public function format(string $locale = 'en'): string { return ''; }\n}\n";
        let m = read(text).unwrap();
        assert_eq!(
            m.properties,
            [
                ("cents".to_owned(), "int".to_owned()),
                ("currency".to_owned(), "string".to_owned())
            ]
        );
        assert_eq!(m.methods[0].0, "format");
        assert_eq!(m.methods[0].1, "(string $locale = 'en'): string");
    }
}
