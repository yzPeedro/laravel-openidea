use super::Entry;
use crate::php::{Kind, Token, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::Project;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub public: bool,
    pub start: usize,
    pub end: usize,
    pub body: (usize, usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Class {
    pub name: String,
    pub extends: Option<String>,
    pub start: usize,
    pub end: usize,
    pub methods: Vec<Method>,
    pub table: Option<String>,
}

fn ident(tokens: &[Token], i: usize) -> Option<&str> {
    match &tokens.get(i)?.kind {
        Kind::Ident(s) => Some(s),
        _ => None,
    }
}

fn matching_close(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (j, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return j;
                }
            }
            _ => {}
        }
    }
    tokens.len()
}

pub fn parse_class(text: &str) -> Option<Class> {
    let tokens = tokenize(text, 0, text.len());
    let at = (0..tokens.len()).find(|&i| {
        ident(&tokens, i).is_some_and(|s| s.eq_ignore_ascii_case("class"))
            && !matches!(
                i.checked_sub(1).map(|k| &tokens[k].kind),
                Some(Kind::DoubleColon)
            )
            && ident(&tokens, i + 1).is_some()
    })?;
    let name_token = &tokens[at + 1];
    let attribute_table = (0..at).find_map(|k| {
        let is_table = tokens[k].kind == Kind::Hash
            && tokens.get(k + 1).is_some_and(|t| t.kind == Kind::Open('['))
            && ident(&tokens, k + 2).is_some_and(|n| n.rsplit('\\').next() == Some("Table"));
        if !is_table {
            return None;
        }
        tokens[k + 3..].iter().take(6).find_map(|t| match &t.kind {
            Kind::Str { value, .. } => Some(value.clone()),
            _ => None,
        })
    });
    let mut class = Class {
        name: ident(&tokens, at + 1)?.to_owned(),
        start: name_token.start,
        end: name_token.end,
        ..Class::default()
    };
    let mut i = at + 2;
    while i < tokens.len() && tokens[i].kind != Kind::Open('{') {
        if ident(&tokens, i).is_some_and(|s| s.eq_ignore_ascii_case("extends")) {
            class.extends =
                ident(&tokens, i + 1).map(|s| s.rsplit('\\').next().unwrap_or(s).to_owned());
        }
        i += 1;
    }
    class.table = attribute_table;
    let body_end = matching_close(&tokens, i);
    let mut depth = 0usize;
    let mut j = i;
    while j < body_end {
        match &tokens[j].kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth = depth.saturating_sub(1),
            Kind::Var(v)
                if depth == 1
                    && v == "table"
                    && tokens.get(j + 1).is_some_and(|t| t.kind == Kind::Assign) =>
            {
                if let Some(Kind::Str { value, .. }) = tokens.get(j + 2).map(|t| &t.kind) {
                    class.table = Some(value.clone());
                }
            }
            Kind::Ident(f) if depth == 1 && f.eq_ignore_ascii_case("function") => {
                if let Some(name) = ident(&tokens, j + 1) {
                    let (name_start, name_end) = (tokens[j + 1].start, tokens[j + 1].end);
                    let mut public = true;
                    let mut k = j;
                    while k > 0 {
                        k -= 1;
                        match ident(&tokens, k) {
                            Some(m)
                                if m.eq_ignore_ascii_case("private")
                                    || m.eq_ignore_ascii_case("protected") =>
                            {
                                public = false
                            }
                            Some(m)
                                if m.eq_ignore_ascii_case("public")
                                    || m.eq_ignore_ascii_case("static")
                                    || m.eq_ignore_ascii_case("final")
                                    || m.eq_ignore_ascii_case("abstract") => {}
                            _ => break,
                        }
                    }
                    let mut b = j + 2;
                    while b < body_end
                        && !matches!(tokens[b].kind, Kind::Open('{') | Kind::Semicolon)
                    {
                        if tokens[b].kind == Kind::Open('(') {
                            b = matching_close(&tokens, b);
                        }
                        b += 1;
                    }
                    let body = if tokens.get(b).is_some_and(|t| t.kind == Kind::Open('{')) {
                        let close = matching_close(&tokens, b);
                        let body = (
                            tokens[b].end,
                            tokens.get(close).map_or(text.len(), |t| t.start),
                        );
                        j = close + 1;
                        body
                    } else {
                        j = b + 1;
                        (0, 0)
                    };
                    class.methods.push(Method {
                        name: name.to_owned(),
                        public,
                        start: name_start,
                        end: name_end,
                        body,
                    });
                    continue;
                }
            }
            _ => {}
        }
        j += 1;
    }
    Some(class)
}

pub fn controller_methods(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut paths = find(project, root, "app/**/Controllers/**/*.php", 5000);
    paths.sort();
    paths.dedup();
    for path in paths {
        let Ok(text) = crate::project::read_listed(project, &path) else {
            continue;
        };
        let Some(class) = parse_class(&text) else {
            continue;
        };
        let lines = Lines::new(&text);
        for m in class
            .methods
            .iter()
            .filter(|m| m.public && !m.name.starts_with("__") || m.name == "__invoke")
        {
            out.push(Entry::new(
                format!("{}.{}", class.name, m.name),
                Some(format!("{}::{}()", class.name, m.name)),
                &path,
                lines.range(&text, m.start, m.end),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classes_methods_and_table() {
        let text = "<?php\nnamespace App\\Models;\n\nclass Post extends \\Illuminate\\Database\\Eloquent\\Model\n{\n    protected $table = 'blog_posts';\n\n    public function author(): BelongsTo\n    {\n        return $this->belongsTo(User::class);\n    }\n\n    protected function secret() { return fn () => 1; }\n\n    public static function booted(): void {}\n}\n";
        let class = parse_class(text).unwrap();
        assert_eq!(class.name, "Post");
        assert_eq!(class.extends.as_deref(), Some("Model"));
        assert_eq!(class.table.as_deref(), Some("blog_posts"));
        let methods: Vec<_> = class
            .methods
            .iter()
            .map(|m| (m.name.as_str(), m.public))
            .collect();
        assert_eq!(
            methods,
            [("author", true), ("secret", false), ("booted", true)]
        );
        let author = &class.methods[0];
        assert_eq!(&text[author.start..author.end], "author");
        assert!(text[author.body.0..author.body.1].contains("belongsTo(User::class)"));
    }
}
