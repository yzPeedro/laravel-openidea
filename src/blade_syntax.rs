use crate::php::{Kind, tokenize};
use openidea_plugin::syntax::{Fold, FoldKind, Highlight, Symbol, SymbolKind, Token};

const PHP_KEYWORDS: [&str; 40] = [
    "and",
    "array",
    "as",
    "break",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "empty",
    "false",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "if",
    "instanceof",
    "isset",
    "list",
    "match",
    "new",
    "null",
    "or",
    "parent",
    "print",
    "return",
    "self",
    "static",
    "switch",
    "throw",
    "true",
    "try",
    "while",
];

const BLOCKS: [&str; 26] = [
    "if",
    "unless",
    "foreach",
    "forelse",
    "for",
    "while",
    "section",
    "push",
    "prepend",
    "pushonce",
    "prependonce",
    "component",
    "slot",
    "auth",
    "guest",
    "can",
    "cannot",
    "canany",
    "isset",
    "empty",
    "switch",
    "once",
    "env",
    "production",
    "error",
    "fragment",
];

fn ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn find(text: &str, from: usize, needle: &str) -> usize {
    text[from.min(text.len())..]
        .find(needle)
        .map_or(text.len(), |i| from + i)
}

fn token(out: &mut Vec<Token>, start: usize, end: usize, kind: Highlight) {
    if end > start && out.last().is_none_or(|t| t.range.end <= start) {
        out.push(Token {
            range: start..end,
            kind,
        });
    }
}

fn php(text: &str, start: usize, end: usize, out: &mut Vec<Token>) {
    let tokens = tokenize(text, start, end);
    for (i, t) in tokens.iter().enumerate() {
        let next_open = tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Open('('));
        let after_member =
            i > 0 && matches!(tokens[i - 1].kind, Kind::ObjectOp | Kind::DoubleColon);
        let kind = match &t.kind {
            Kind::Str { closed, .. } => {
                let s = t.start.saturating_sub(1).max(start);
                let e = if *closed { (t.end + 1).min(end) } else { t.end };
                token(out, s, e, Highlight::String);
                continue;
            }
            Kind::Num(_) => Highlight::Number,
            Kind::Var(v) if v == "this" => Highlight::SelfVariable,
            Kind::Var(_) => Highlight::Variable,
            Kind::Ident(n) if PHP_KEYWORDS.contains(&n.to_ascii_lowercase().as_str()) => {
                Highlight::Keyword
            }
            Kind::Ident(_) if next_open => Highlight::FunctionCall,
            Kind::Ident(_) if after_member => Highlight::Property,
            Kind::Ident(n) if n.starts_with(|c: char| c.is_ascii_uppercase() || c == '\\') => {
                Highlight::Type
            }
            Kind::Ident(_) => Highlight::Constant,
            Kind::ObjectOp | Kind::DoubleColon | Kind::DoubleArrow | Kind::Assign | Kind::Dot => {
                Highlight::Operator
            }
            _ => continue,
        };
        token(out, t.start, t.end, kind);
    }
}

fn matching_paren(text: &str, open: usize) -> usize {
    let b = text.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            q @ (b'\'' | b'"') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'\n' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

fn tag(text: &str, at: usize, out: &mut Vec<Token>) -> usize {
    let b = text.as_bytes();
    let mut i = at + 1;
    if b.get(i) == Some(&b'/') {
        i += 1;
    }
    while i < b.len() && (ident_byte(b[i]) || matches!(b[i], b'-' | b':' | b'.')) {
        i += 1;
    }
    let component = text[at..i].trim_start_matches(['<', '/']).starts_with("x-")
        || text[at..i].contains("livewire:");
    token(
        out,
        at,
        i,
        if component {
            Highlight::Type
        } else {
            Highlight::Tag
        },
    );
    while i < b.len() {
        match b[i] {
            b'>' => {
                token(out, i, i + 1, Highlight::Tag);
                return i + 1;
            }
            b'/' if b.get(i + 1) == Some(&b'>') => {
                token(out, i, i + 2, Highlight::Tag);
                return i + 2;
            }
            b'{' if text[i..].starts_with("{{") => {
                let close = find(text, i + 2, "}}");
                token(out, i, i + 2, Highlight::Macro);
                php(text, i + 2, close, out);
                token(out, close, (close + 2).min(text.len()), Highlight::Macro);
                i = (close + 2).min(text.len());
            }
            b'@' if b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) => {
                i = directive(text, i, out)
            }
            c if ident_byte(c) || matches!(c, b':' | b'@' | b'-' | b'.') => {
                let s = i;
                while i < b.len() && (ident_byte(b[i]) || matches!(b[i], b':' | b'@' | b'-' | b'.'))
                {
                    i += 1;
                }
                let name = &text[s..i];
                token(out, s, i, Highlight::Attribute);
                if b.get(i) == Some(&b'=') {
                    token(out, i, i + 1, Highlight::Operator);
                    i += 1;
                    if let Some(&q @ (b'"' | b'\'')) = b.get(i) {
                        let close = text[i + 1..].find(q as char).map_or(b.len(), |c| i + 1 + c);
                        let bound = name.starts_with(':') || name.starts_with("x-bind:");
                        if bound {
                            token(out, i, i + 1, Highlight::String);
                            php(text, i + 1, close, out);
                            token(out, close, (close + 1).min(b.len()), Highlight::String);
                        } else if text[i + 1..close].contains("{{") {
                            token(out, i, i + 1, Highlight::String);
                            let mut j = i + 1;
                            while let Some(open) = text[j..close].find("{{").map(|o| j + o) {
                                token(out, j, open, Highlight::String);
                                let end = find(text, open + 2, "}}").min(close);
                                token(out, open, open + 2, Highlight::Macro);
                                php(text, open + 2, end, out);
                                token(out, end, (end + 2).min(close), Highlight::Macro);
                                j = (end + 2).min(close);
                            }
                            token(out, j, (close + 1).min(b.len()), Highlight::String);
                        } else {
                            token(out, i, (close + 1).min(b.len()), Highlight::String);
                        }
                        i = (close + 1).min(b.len());
                    }
                }
            }
            _ => i += 1,
        }
    }
    i
}

fn directive(text: &str, at: usize, out: &mut Vec<Token>) -> usize {
    let b = text.as_bytes();
    let mut i = at + 1;
    while i < b.len() && ident_byte(b[i]) {
        i += 1;
    }
    let name = &text[at + 1..i];
    token(out, at, i, Highlight::Macro);
    if name == "php" && !text[i..].trim_start_matches([' ', '\t']).starts_with('(') {
        let end = find(text, i, "@endphp");
        php(text, i, end, out);
        if end < text.len() {
            token(out, end, end + 7, Highlight::Macro);
        }
        return (end + 7).min(text.len());
    }
    if name == "verbatim" {
        let end = find(text, i, "@endverbatim");
        if end < text.len() {
            token(out, end, end + 12, Highlight::Macro);
        }
        return (end + 12).min(text.len());
    }
    let mut j = i;
    while j < b.len() && matches!(b[j], b' ' | b'\t') {
        j += 1;
    }
    if b.get(j) == Some(&b'(') {
        let close = matching_paren(text, j);
        token(out, j, j + 1, Highlight::Punctuation);
        php(text, j + 1, close.min(text.len()), out);
        if b.get(close) == Some(&b')') {
            token(out, close, close + 1, Highlight::Punctuation);
            return close + 1;
        }
        return close;
    }
    i
}

pub fn highlights(text: &str) -> Vec<Token> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let rest = &text[i..];
        if rest.starts_with("{{--") {
            let end = (find(text, i + 4, "--}}") + 4).min(text.len());
            token(&mut out, i, end, Highlight::Comment);
            i = end;
        } else if rest.starts_with("<!--") {
            let end = (find(text, i + 4, "-->") + 3).min(text.len());
            token(&mut out, i, end, Highlight::Comment);
            i = end;
        } else if rest.starts_with("@{{") {
            i += 3;
        } else if rest.starts_with("{!!") || rest.starts_with("{{") {
            let (open, close_tag) = if rest.starts_with("{!!") {
                (3, "!!}")
            } else {
                (2, "}}")
            };
            let close = find(text, i + open, close_tag);
            token(&mut out, i, i + open, Highlight::Macro);
            php(text, i + open, close, &mut out);
            token(
                &mut out,
                close,
                (close + close_tag.len()).min(text.len()),
                Highlight::Macro,
            );
            i = (close + close_tag.len()).min(text.len());
        } else if rest.starts_with("<?php") || rest.starts_with("<?=") {
            let open = if rest.starts_with("<?php") { 5 } else { 3 };
            let close = find(text, i + open, "?>");
            token(&mut out, i, i + open, Highlight::Macro);
            php(text, i + open, close, &mut out);
            token(
                &mut out,
                close,
                (close + 2).min(text.len()),
                Highlight::Macro,
            );
            i = (close + 2).min(text.len());
        } else if b[i] == b'@'
            && (i == 0 || !ident_byte(b[i - 1]) && b[i - 1] != b'@')
            && b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic())
        {
            i = directive(text, i, &mut out);
        } else if b[i] == b'<'
            && b.get(i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'/')
        {
            let raw = ["<script", "<style"]
                .iter()
                .find(|t| rest.to_ascii_lowercase().starts_with(*t))
                .copied();
            i = tag(text, i, &mut out);
            if let Some(raw) = raw {
                let close = format!("</{}", &raw[1..]);
                i = text[i..]
                    .to_ascii_lowercase()
                    .find(&close)
                    .map_or(text.len(), |c| i + c);
            }
        } else {
            i += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

type Directive = (String, usize, usize, Option<(String, usize, usize)>);

fn directives(text: &str) -> Vec<Directive> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"{{--") {
            i = find(text, i, "--}}");
            continue;
        }
        if b[i] == b'@'
            && (i == 0 || !ident_byte(b[i - 1]) && b[i - 1] != b'@')
            && b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic())
        {
            let mut j = i + 1;
            while j < b.len() && ident_byte(b[j]) {
                j += 1;
            }
            let name = text[i + 1..j].to_ascii_lowercase();
            let mut arg = None;
            if b.get(j) == Some(&b'(') {
                let close = matching_paren(text, j);
                let inner = &text[j + 1..close.min(text.len())];
                if let Some(q) = inner.find(['\'', '"']) {
                    let quote = inner.as_bytes()[q] as char;
                    if let Some(e) = inner[q + 1..].find(quote) {
                        let s = j + 1 + q + 1;
                        arg = Some((inner[q + 1..q + 1 + e].to_owned(), s, s + e));
                    }
                }
                let single_arg_section = name == "section" && inner.contains(',');
                out.push((
                    if single_arg_section {
                        "section-inline".into()
                    } else {
                        name
                    },
                    i,
                    close.min(text.len()),
                    arg,
                ));
                i = close.min(text.len());
                continue;
            }
            out.push((name, i, j, arg));
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

fn line_of(text: &str, at: usize) -> u32 {
    text[..at.min(text.len())].matches('\n').count() as u32
}

pub fn folding(text: &str) -> Vec<Fold> {
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut out = Vec::new();
    for (name, start, _, _) in directives(text) {
        if BLOCKS.contains(&name.as_str()) || name == "php" {
            stack.push((name, start));
            continue;
        }
        let closes = name.strip_prefix("end").map(str::to_owned).or_else(|| {
            matches!(name.as_str(), "show" | "stop" | "overwrite" | "append")
                .then(|| "section".to_owned())
        });
        let Some(closes) = closes else { continue };
        if let Some(pos) = stack.iter().rposition(|(n, _)| *n == closes) {
            let (_, open) = stack.remove(pos);
            stack.truncate(pos);
            let (s, e) = (line_of(text, open), line_of(text, start));
            if e > s {
                out.push(Fold {
                    start_line: s,
                    end_line: e,
                    kind: FoldKind::Region,
                });
            }
        }
    }
    out
}

pub fn outline(text: &str) -> Vec<Symbol> {
    directives(text)
        .into_iter()
        .filter_map(|(name, start, end, arg)| {
            let kind = match name.as_str() {
                "section" | "section-inline" => SymbolKind::Module,
                "push" | "prepend" | "pushonce" => SymbolKind::Event,
                "slot" => SymbolKind::Field,
                "extends" | "include" | "component" | "livewire" => SymbolKind::Package,
                _ => return None,
            };
            let (label, s, e) = arg?;
            Some(Symbol {
                name: label,
                detail: Some(format!("@{}", name.trim_end_matches("-inline"))),
                kind,
                range: start..end,
                name_range: s..e,
                parent: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_after_multibyte_text() {
        let text = "<p>Price — {{ $total }}</p>
@if ($a)
<p>—</p>
@endif
";
        let names: Vec<String> = directives(text).into_iter().map(|d| d.0).collect();
        assert_eq!(names, ["if", "endif"]);
    }

    fn kinds(text: &str) -> Vec<(&str, &'static str)> {
        highlights(text)
            .into_iter()
            .map(|t| (&text[t.range.clone()], t.kind.name()))
            .collect()
    }

    #[test]
    fn highlights_html_directives_and_echoes() {
        let text = "<td class=\"panel-item\">\n    @section('title', $page)\n{{ Illuminate\\Mail\\Markdown::parse($slot) }}\n<x-alert :type=\"$kind\" />\n{{-- note --}}\n</td>\n";
        let got = kinds(text);
        assert!(got.contains(&("<td", "tag")));
        assert!(got.contains(&("class", "attribute")));
        assert!(got.contains(&("\"panel-item\"", "string")));
        assert!(got.contains(&("@section", "macro")));
        assert!(got.contains(&("'title'", "string")));
        assert!(got.contains(&("$page", "variable")));
        assert!(got.contains(&("{{", "macro")));
        assert!(got.contains(&("parse", "function-call")));
        assert!(got.contains(&("<x-alert", "type")));
        assert!(got.contains(&(":type", "attribute")));
        assert!(got.contains(&("$kind", "variable")));
        assert!(got.contains(&("{{-- note --}}", "comment")));
        assert!(got.contains(&("</td", "tag")));
        let tokens = highlights(text);
        assert!(
            tokens
                .windows(2)
                .all(|w| w[0].range.end <= w[1].range.start)
        );
    }

    #[test]
    fn folds_blocks_and_lists_sections() {
        let text = "@section('content')\n  @if($a)\n    x\n  @endif\n@endsection\n@push('scripts')\n<script></script>\n@endpush\n";
        let folds: Vec<_> = folding(text)
            .into_iter()
            .map(|f| (f.start_line, f.end_line))
            .collect();
        assert!(folds.contains(&(1, 3)));
        assert!(folds.contains(&(0, 4)));
        assert!(folds.contains(&(5, 7)));
        let names: Vec<_> = outline(text).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["content", "scripts"]);
    }
}
