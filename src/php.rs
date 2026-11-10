#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Str { value: String, closed: bool },
    Ident(String),
    Var(String),
    Num(String),
    DoubleArrow,
    ObjectOp,
    DoubleColon,
    Colon,
    At,
    Open(char),
    Close(char),
    Comma,
    Semicolon,
    Assign,
    Dot,
    Hash,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    Function,
    Static,
    Method,
    Directive,
    Property,
    Return,
    Attribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callee {
    pub kind: CallKind,
    pub class: Option<String>,
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringArg {
    pub callee: Callee,
    pub arg: usize,
    pub in_array: bool,
    pub is_key: bool,
    pub key: Option<String>,
    pub head: Option<String>,
    pub function: Option<String>,
    pub class_ref: Option<String>,
    pub param: Option<String>,
    pub call_start: usize,
    pub value: String,
    pub start: usize,
    pub end: usize,
    pub closed: bool,
    pub concat: bool,
    pub after_arrow: bool,
}

fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'\\' || b >= 0x80
}

fn ident_byte(b: u8) -> bool {
    ident_start(b) || b.is_ascii_digit()
}

fn ident_end(b: &[u8], mut i: usize, limit: usize) -> usize {
    while i < limit && ident_byte(b[i]) {
        i += 1;
    }
    i
}

fn find(b: &[u8], from: usize, needle: &[u8], limit: usize) -> Option<usize> {
    let limit = limit.min(b.len());
    if from >= limit || needle.len() > limit - from {
        return None;
    }
    (from..=limit - needle.len()).find(|&i| &b[i..i + needle.len()] == needle)
}

fn line_end(b: &[u8], from: usize, limit: usize) -> usize {
    find(b, from, b"\n", limit).unwrap_or(limit.min(b.len()))
}

fn string(text: &str, at: usize, limit: usize) -> (Token, usize) {
    let b = text.as_bytes();
    let quote = b[at];
    let mut value = String::new();
    let mut i = at + 1;
    while i < limit {
        match b[i] {
            b'\\' if i + 1 < limit && (b[i + 1] == quote || b[i + 1] == b'\\') => {
                value.push(b[i + 1] as char);
                i += 2;
            }
            c if c == quote => {
                let token = Token {
                    kind: Kind::Str {
                        value,
                        closed: true,
                    },
                    start: at + 1,
                    end: i,
                };
                return (token, i + 1);
            }
            _ => {
                let ch = text[i..].chars().next().unwrap_or('\u{fffd}');
                value.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    let end = limit.min(b.len());
    (
        Token {
            kind: Kind::Str {
                value,
                closed: false,
            },
            start: at + 1,
            end,
        },
        end,
    )
}

fn heredoc(b: &[u8], at: usize, limit: usize) -> usize {
    let mut i = at + 3;
    while i < limit && (b[i] == b' ' || b[i] == b'"' || b[i] == b'\'') {
        i += 1;
    }
    let s = i;
    let e = ident_end(b, i, limit);
    if e == s {
        return at + 3;
    }
    let label = &b[s..e];
    let mut line = line_end(b, e, limit);
    while line < limit {
        let mut j = line + 1;
        while j < limit && (b[j] == b' ' || b[j] == b'\t') {
            j += 1;
        }
        if b[j.min(b.len())..].starts_with(label)
            && !b.get(j + label.len()).is_some_and(|c| ident_byte(*c))
        {
            return j + label.len();
        }
        line = line_end(b, line + 1, limit);
    }
    limit
}

pub fn tokenize(text: &str, from: usize, limit: usize) -> Vec<Token> {
    let b = text.as_bytes();
    let limit = limit.min(b.len());
    let mut out = Vec::new();
    let mut i = from;
    let push = |out: &mut Vec<Token>, kind: Kind, start: usize, end: usize| {
        out.push(Token { kind, start, end })
    };
    while i < limit {
        let c = b[i];
        let next = b.get(i + 1).copied();
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'/' if next == Some(b'/') => i = line_end(b, i, limit),
            b'/' if next == Some(b'*') => i = find(b, i + 2, b"*/", limit).map_or(limit, |j| j + 2),
            b'#' if next == Some(b'[') => {
                push(&mut out, Kind::Hash, i, i + 1);
                i += 1;
            }
            b'#' => i = line_end(b, i, limit),
            b'\'' | b'"' => {
                let (token, after) = string(text, i, limit);
                out.push(token);
                i = after;
            }
            b'<' if b[i..].starts_with(b"<<<") => i = heredoc(b, i, limit),
            b'$' if next.is_some_and(ident_start) && !next.is_some_and(|n| n == b'\\') => {
                let e = ident_end(b, i + 1, limit);
                push(&mut out, Kind::Var(text[i + 1..e].to_owned()), i, e);
                i = e;
            }
            b'=' if next == Some(b'>') => {
                push(&mut out, Kind::DoubleArrow, i, i + 2);
                i += 2;
            }
            b'=' if next != Some(b'=') => {
                push(&mut out, Kind::Assign, i, i + 1);
                i += 1;
            }
            b'-' if next == Some(b'>') => {
                push(&mut out, Kind::ObjectOp, i, i + 2);
                i += 2;
            }
            b'?' if b[i..].starts_with(b"?->") => {
                push(&mut out, Kind::ObjectOp, i, i + 3);
                i += 3;
            }
            b':' if next == Some(b':') => {
                push(&mut out, Kind::DoubleColon, i, i + 2);
                i += 2;
            }
            b':' => {
                push(&mut out, Kind::Colon, i, i + 1);
                i += 1;
            }
            b'@' => {
                push(&mut out, Kind::At, i, i + 1);
                i += 1;
            }
            b'(' | b'[' | b'{' => {
                push(&mut out, Kind::Open(c as char), i, i + 1);
                i += 1;
            }
            b')' | b']' | b'}' => {
                push(&mut out, Kind::Close(c as char), i, i + 1);
                i += 1;
            }
            b',' => {
                push(&mut out, Kind::Comma, i, i + 1);
                i += 1;
            }
            b';' => {
                push(&mut out, Kind::Semicolon, i, i + 1);
                i += 1;
            }
            b'.' if !next.is_some_and(|n| n.is_ascii_digit()) => {
                push(&mut out, Kind::Dot, i, i + 1);
                i += 1;
            }
            _ if c.is_ascii_digit() => {
                let e = ident_end(b, i, limit);
                push(&mut out, Kind::Num(text[i..e].to_owned()), i, e);
                i = e;
            }
            _ if ident_start(c) => {
                let e = ident_end(b, i, limit);
                push(&mut out, Kind::Ident(text[i..e].to_owned()), i, e);
                i = e;
            }
            _ => {
                push(&mut out, Kind::Other, i, i + 1);
                i += 1;
            }
        }
    }
    out
}

fn last_segment(name: &str) -> String {
    name.rsplit('\\')
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase()
}

fn receiver(tokens: &[Token], i: usize) -> Option<String> {
    let kind = |k: usize| {
        i.checked_sub(k)
            .and_then(|k| tokens.get(k))
            .map(|t| &t.kind)
    };
    match kind(3)? {
        Kind::Var(v) => Some(format!("${v}")),
        Kind::Ident(p) => match (kind(4)?, kind(5)?) {
            (Kind::ObjectOp, Kind::Var(v)) if v == "this" => Some(format!("$this->{p}")),
            _ => None,
        },
        Kind::Close(')') => match (kind(4)?, kind(5)?) {
            (Kind::Open('('), Kind::Ident(f)) => Some(format!("{}()", last_segment(f))),
            _ => None,
        },
        _ => None,
    }
}

pub fn callee_before(tokens: &[Token], i: usize) -> Option<Callee> {
    let name_token = tokens.get(i.checked_sub(1)?)?;
    let Kind::Ident(name) = &name_token.kind else {
        return None;
    };
    let namespace = name
        .trim_start_matches('\\')
        .rsplit_once('\\')
        .map(|(ns, _)| ns.to_ascii_lowercase());
    let name = last_segment(name);
    let prev = i.checked_sub(2).and_then(|k| tokens.get(k));
    let (kind, class) = match prev.map(|t| &t.kind) {
        Some(Kind::ObjectOp) => (CallKind::Method, receiver(tokens, i)),
        Some(Kind::DoubleColon) => {
            let class = match i
                .checked_sub(3)
                .and_then(|k| tokens.get(k))
                .map(|t| &t.kind)
            {
                Some(Kind::Ident(c)) => Some(last_segment(c)),
                _ => None,
            };
            (CallKind::Static, class)
        }
        Some(Kind::At) if prev.is_some_and(|p| p.end == name_token.start) => {
            (CallKind::Directive, None)
        }
        Some(Kind::Open('['))
            if i.checked_sub(3)
                .and_then(|k| tokens.get(k))
                .is_some_and(|t| t.kind == Kind::Hash) =>
        {
            (CallKind::Attribute, None)
        }
        _ => (CallKind::Function, None),
    };
    let namespace = if matches!(kind, CallKind::Function | CallKind::Attribute) {
        namespace
    } else {
        None
    };
    Some(Callee {
        kind,
        class,
        name,
        namespace,
    })
}

struct Frame {
    callee: Option<Callee>,
    head: Option<String>,
    arg: usize,
    bracket: char,
    key: Option<String>,
    class_ref: Option<String>,
    named: Option<String>,
    start: usize,
}

fn close_index(tokens: &[Token], close: usize) -> Option<usize> {
    let mut depth = 0usize;
    for j in (0..=close).rev() {
        match tokens[j].kind {
            Kind::Close(_) => depth += 1,
            Kind::Open(_) => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn chain(tokens: &[Token], open: usize) -> Option<(String, Vec<String>)> {
    let mut j = open.checked_sub(1)?;
    let mut steps = Vec::new();
    loop {
        let Kind::Ident(name) = &tokens.get(j)?.kind else {
            return None;
        };
        if j + 1 != open {
            steps.push(name.clone());
        }
        match tokens.get(j.checked_sub(1)?)?.kind {
            Kind::DoubleColon => {
                let head = match &tokens.get(j.checked_sub(2)?)?.kind {
                    Kind::Ident(c) => c.rsplit('\\').next().unwrap_or(c).to_owned(),
                    _ => return None,
                };
                steps.reverse();
                return Some((head, steps));
            }
            Kind::ObjectOp => {
                let before = j.checked_sub(2)?;
                match &tokens.get(before)?.kind {
                    Kind::Close(')') => j = close_index(tokens, before)?.checked_sub(1)?,
                    Kind::Var(v) => {
                        steps.reverse();
                        return Some((format!("${v}"), steps));
                    }
                    Kind::Ident(_) => j = before,
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
}

pub fn chain_head(tokens: &[Token], open: usize) -> Option<String> {
    let mut j = open.checked_sub(1)?;
    loop {
        let Kind::Ident(_) = tokens.get(j)?.kind else {
            return None;
        };
        match tokens.get(j.checked_sub(1)?)?.kind {
            Kind::DoubleColon => {
                return match &tokens.get(j.checked_sub(2)?)?.kind {
                    Kind::Ident(c) => Some(c.rsplit('\\').next().unwrap_or(c).to_owned()),
                    _ => None,
                };
            }
            Kind::ObjectOp => {
                let before = j.checked_sub(2)?;
                match &tokens.get(before)?.kind {
                    Kind::Close(')') => j = close_index(tokens, before)?.checked_sub(1)?,
                    Kind::Var(v) => return Some(format!("${v}")),
                    Kind::Ident(_) => j = before,
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
}

fn array_context(tokens: &[Token], i: usize, function: Option<&str>) -> Option<Callee> {
    let prev = i.checked_sub(1).map(|k| &tokens[k].kind);
    match prev {
        Some(Kind::Assign) => match i.checked_sub(2).map(|k| &tokens[k].kind) {
            Some(Kind::Var(v)) => Some(Callee {
                kind: CallKind::Property,
                class: None,
                name: v.to_ascii_lowercase(),
                namespace: None,
            }),
            _ => None,
        },
        Some(Kind::Ident(r)) if r.eq_ignore_ascii_case("return") => Some(Callee {
            kind: CallKind::Return,
            class: None,
            name: function.unwrap_or_default().to_ascii_lowercase(),
            namespace: None,
        }),
        _ => None,
    }
}

pub fn string_args(tokens: &[Token]) -> Vec<StringArg> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut functions: Vec<(String, usize)> = Vec::new();
    let mut pending_function: Option<String> = None;
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        match &t.kind {
            Kind::Ident(f) if f.eq_ignore_ascii_case("function") => {
                pending_function = match tokens.get(i + 1).map(|n| &n.kind) {
                    Some(Kind::Ident(name)) => Some(name.clone()),
                    _ => None,
                };
            }
            Kind::Open(c) => {
                let function = functions.last().map(|(f, _)| f.as_str());
                let (callee, head, bracket) = match c {
                    '(' => match callee_before(tokens, i) {
                        Some(cl) if cl.kind == CallKind::Function && cl.name == "array" => {
                            (array_context(tokens, i - 1, function), None, '[')
                        }
                        Some(cl) => {
                            let head = match cl.kind {
                                CallKind::Static => match &tokens[i - 3].kind {
                                    Kind::Ident(c) => {
                                        Some(c.rsplit('\\').next().unwrap_or(c).to_owned())
                                    }
                                    _ => None,
                                },
                                CallKind::Method => chain_head(tokens, i),
                                _ => None,
                            };
                            (Some(cl), head, '(')
                        }
                        None => (None, None, '('),
                    },
                    '[' => (array_context(tokens, i, function), None, '['),
                    _ => {
                        if let Some(name) = pending_function.take() {
                            functions.push((name, frames.len()));
                        }
                        (None, None, '{')
                    }
                };
                let named = match (
                    i.checked_sub(1).map(|k| &tokens[k].kind),
                    i.checked_sub(2).map(|k| &tokens[k].kind),
                ) {
                    (Some(Kind::Colon), Some(Kind::Ident(n))) if bracket == '[' => Some(n.clone()),
                    _ => None,
                };
                frames.push(Frame {
                    callee,
                    head,
                    arg: 0,
                    bracket,
                    key: None,
                    class_ref: None,
                    named,
                    start: t.start,
                });
            }
            Kind::Ident(c)
                if c == "class"
                    && i >= 2
                    && tokens[i - 1].kind == Kind::DoubleColon
                    && let Kind::Ident(name) = &tokens[i - 2].kind =>
            {
                if let Some(top) = frames.last_mut() {
                    top.class_ref = Some(name.rsplit('\\').next().unwrap_or(name).to_owned());
                }
            }
            Kind::Close(_) => {
                frames.pop();
                if functions
                    .last()
                    .is_some_and(|(_, depth)| *depth == frames.len())
                {
                    functions.pop();
                }
            }
            Kind::Semicolon => pending_function = None,
            Kind::Comma => {
                if let Some(f) = frames.last_mut() {
                    f.arg += 1;
                    f.key = None;
                }
            }
            Kind::Str { value, closed } => {
                let is_key = tokens
                    .get(i + 1)
                    .is_some_and(|n| n.kind == Kind::DoubleArrow);
                let mut depth = 0;
                let mut key = None;
                let mut found = None;
                let mut param = match (
                    i.checked_sub(1).map(|k| &tokens[k].kind),
                    i.checked_sub(2).map(|k| &tokens[k].kind),
                ) {
                    (Some(Kind::Colon), Some(Kind::Ident(n))) => Some(n.clone()),
                    _ => None,
                };
                for f in frames.iter().rev() {
                    match f.bracket {
                        '[' => {
                            if f.named.is_some() {
                                param = f.named.clone();
                            }
                            if key.is_none() && !(depth == 0 && is_key) {
                                key = f.key.clone();
                            }
                            depth += 1;
                            if f.callee.is_some() {
                                found = Some(f);
                                break;
                            }
                        }
                        '(' => {
                            found = Some(f);
                            break;
                        }
                        _ => break,
                    }
                }
                let class_ref = frames.last().and_then(|f| f.class_ref.clone());
                let found = found.and_then(|f| {
                    Some((f.callee.clone()?, f.head.clone(), f.arg, f.bracket, f.start))
                });
                if is_key && let Some(top) = frames.last_mut() {
                    top.key = Some(value.clone());
                }
                let Some((callee, head, arg, bracket, call_start)) = found else {
                    continue;
                };
                let in_array = bracket == '(' && depth > 0 || bracket == '[' && depth > 1;
                out.push(StringArg {
                    callee,
                    arg,
                    in_array,
                    is_key,
                    key,
                    head,
                    function: functions.last().map(|(f, _)| f.clone()),
                    class_ref,
                    param,
                    call_start,
                    value: value.clone(),
                    start: t.start,
                    end: t.end,
                    closed: *closed,
                    concat: i
                        .checked_sub(1)
                        .is_some_and(|k| tokens[k].kind == Kind::Dot)
                        || tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Dot),
                    after_arrow: i
                        .checked_sub(1)
                        .is_some_and(|k| tokens[k].kind == Kind::DoubleArrow),
                });
            }
            _ => {}
        }
    }
    out
}

pub fn is_blade(path: &str) -> bool {
    path.ends_with(".blade.php")
}

pub fn regions(text: &str, blade: bool) -> Vec<(usize, usize)> {
    if !blade {
        return vec![(0, text.len())];
    }
    let b = text.as_bytes();
    let len = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < len {
        let rest = &b[i..];
        if rest.starts_with(b"{{--") {
            i = find(b, i + 4, b"--}}", len).map_or(len, |j| j + 4);
        } else if rest.starts_with(b"{!!") {
            let e = find(b, i + 3, b"!!}", len).unwrap_or(len);
            out.push((i + 3, e));
            i = e;
        } else if rest.starts_with(b"{{") {
            let e = find(b, i + 2, b"}}", len).unwrap_or(len);
            out.push((i + 2, e));
            i = e;
        } else if rest.starts_with(b"<?php") {
            let e = find(b, i + 5, b"?>", len).unwrap_or(len);
            out.push((i + 5, e));
            i = e;
        } else if rest.starts_with(b"@php") && !b.get(i + 4).is_some_and(|c| ident_byte(*c)) {
            let e = find(b, i + 4, b"@endphp", len).unwrap_or(len);
            out.push((i + 4, e));
            i = e;
        } else if b[i] == b'@'
            && (i == 0 || !ident_byte(b[i - 1]) && b[i - 1] != b'@')
            && b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic())
        {
            let e = line_end(b, i, len);
            out.push((i, e));
            i = e;
        } else {
            i += 1;
        }
    }
    out
}

pub fn args_in(text: &str, blade: bool) -> Vec<StringArg> {
    regions(text, blade)
        .into_iter()
        .flat_map(|(s, e)| string_args(&tokenize(text, s, e)))
        .collect()
}

fn region_at(text: &str, blade: bool, cursor: usize) -> Option<(usize, usize)> {
    regions(text, blade)
        .into_iter()
        .find(|&(s, e)| s <= cursor && cursor <= e)
}

pub fn open_arg_at(text: &str, blade: bool, cursor: usize) -> Option<StringArg> {
    let (start, _) = region_at(text, blade, cursor)?;
    let mut arg = string_args(&tokenize(text, start, cursor))
        .into_iter()
        .rev()
        .find(|a| !a.closed && a.end == cursor)?;
    let quote = text[..arg.start].chars().last()?;
    let rest = &text[cursor..];
    if let Some(close) = rest.find(quote)
        && !rest[..close].contains('\n')
        && rest[close + 1..].trim_start().starts_with("=>")
    {
        arg.is_key = true;
    }
    Some(arg)
}

pub fn arg_at(text: &str, blade: bool, cursor: usize) -> Option<StringArg> {
    let (start, end) = region_at(text, blade, cursor)?;
    string_args(&tokenize(text, start, end))
        .into_iter()
        .find(|a| a.start <= cursor && cursor <= a.end)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayKey {
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub value: Option<String>,
}

pub fn array_keys(tokens: &[Token]) -> Vec<ArrayKey> {
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut pending: Option<String> = None;
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        match &t.kind {
            Kind::Open(_) => stack.push(pending.take()),
            Kind::Close(_) => {
                stack.pop();
            }
            Kind::Str { value: key, .. }
                if tokens
                    .get(i + 1)
                    .is_some_and(|n| n.kind == Kind::DoubleArrow) =>
            {
                let mut path: Vec<&str> = stack.iter().flatten().map(String::as_str).collect();
                path.push(key);
                let next = tokens.get(i + 2).map(|n| &n.kind);
                let nested = matches!(next, Some(Kind::Open('[')))
                    || matches!(next, Some(Kind::Ident(a)) if a.eq_ignore_ascii_case("array"));
                let call = tokens.get(i + 3).is_some_and(|n| n.kind == Kind::Open('('));
                let value = match next {
                    Some(Kind::Str { value, .. }) => Some(format!("'{value}'")),
                    Some(Kind::Num(n)) => Some(n.clone()),
                    Some(Kind::Ident(f)) if call && f.eq_ignore_ascii_case("env") => {
                        match tokens.get(i + 4).map(|n| &n.kind) {
                            Some(Kind::Str { value, .. }) => Some(format!("env('{value}')")),
                            _ => None,
                        }
                    }
                    Some(Kind::Ident(v)) if !nested && !call => Some(v.clone()),
                    _ => None,
                };
                out.push(ArrayKey {
                    path: path.join("."),
                    start: t.start,
                    end: t.end,
                    value,
                });
                if nested {
                    pending = Some(key.clone());
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_values(text: &str, blade: bool) -> Vec<(String, usize, String)> {
        args_in(text, blade)
            .into_iter()
            .map(|a| (a.callee.name, a.arg, a.value))
            .collect()
    }

    #[test]
    fn finds_string_arguments_with_their_calls() {
        let text = "<?php\nreturn view('users.index', ['a' => config('app.name')]);\nRoute::view('/w', \"welcome\");\n$r->route('home'); // route('no')\n";
        assert_eq!(
            arg_values(text, false),
            [
                ("view".into(), 0, "users.index".into()),
                ("view".into(), 1, "a".into()),
                ("config".into(), 0, "app.name".into()),
                ("view".into(), 0, "/w".into()),
                ("view".into(), 1, "welcome".into()),
                ("route".into(), 0, "home".into()),
            ]
        );
        let args = args_in(text, false);
        assert!(args[1].is_key);
        assert_eq!(args[2].key, None);
        assert_eq!(args[4].callee.kind, CallKind::Static);
        assert_eq!(args[4].callee.class.as_deref(), Some("route"));
        assert_eq!(args[5].callee.kind, CallKind::Method);
        assert_eq!(args[5].head.as_deref(), Some("$r"));
    }

    #[test]
    fn reads_blade_regions_only() {
        let text = "<p>Don't</p>\n@include('partials.nav')\n{{ route('home') }} mail@include('x')\n{{-- view('no') --}}\n@includeFirst(['a', 'b'])\n";
        let args = args_in(text, true);
        let names: Vec<_> = args
            .iter()
            .map(|a| (a.callee.name.as_str(), a.value.as_str(), a.in_array))
            .collect();
        assert_eq!(
            names,
            [
                ("include", "partials.nav", false),
                ("route", "home", false),
                ("includefirst", "a", true),
                ("includefirst", "b", true)
            ]
        );
        assert_eq!(args[0].callee.kind, CallKind::Directive);
    }

    #[test]
    fn finds_the_open_string_at_the_cursor() {
        let text = "<?php\nconfig('app.na')";
        let cursor = text.find("na'").unwrap() + 2;
        let arg = open_arg_at(text, false, cursor).unwrap();
        assert_eq!(
            (arg.callee.name.as_str(), arg.value.as_str()),
            ("config", "app.na")
        );
        let inside = arg_at(text, false, cursor).unwrap();
        assert!(inside.closed);
    }

    #[test]
    fn collects_nested_array_keys() {
        let text = "<?php\nreturn [\n  'name' => env('APP_NAME', 'Laravel'),\n  'debug' => true,\n  'connections' => [\n    'mysql' => array('host' => '127.0.0.1'),\n  ],\n];\n";
        let keys: Vec<_> = array_keys(&tokenize(text, 0, text.len()))
            .into_iter()
            .map(|k| (k.path, k.value))
            .collect();
        assert_eq!(
            keys,
            [
                ("name".into(), Some("env('APP_NAME')".into())),
                ("debug".into(), Some("true".into())),
                ("connections".into(), None),
                ("connections.mysql".into(), None),
                ("connections.mysql.host".into(), Some("'127.0.0.1'".into())),
            ]
        );
    }

    #[test]
    fn skips_comments_and_heredocs() {
        let text =
            "<?php\n/* view('a') */\n$x = <<<EOT\nview('b')\nEOT;\n# view('c')\nview('d');\n";
        assert_eq!(arg_values(text, false), [("view".into(), 0, "d".into())]);
    }
}
