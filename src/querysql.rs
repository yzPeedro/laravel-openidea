#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    pub table: String,
    pub soft_deletes: bool,
    pub scopes: Vec<Scope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub name: String,
    pub query: String,
    pub params: Vec<String>,
    pub body: String,
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

pub fn scopes_of(source: &str) -> Vec<Scope> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = source[at..].find("function ") {
        let start = at + i;
        at = start + 9;
        let rest = &source[at..];
        let name_len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let raw = &rest[..name_len];
        let before = &source[source[..start].rfind('}').map_or(0, |n| n + 1)..start];
        let name = if let Some(n) = raw
            .strip_prefix("scope")
            .filter(|n| n.starts_with(|c: char| c.is_ascii_uppercase()))
        {
            let mut n = n.to_owned();
            n[..1].make_ascii_lowercase();
            n
        } else if before.contains("#[Scope]") || before.contains("Attributes\\Scope]") {
            raw.to_owned()
        } else {
            continue;
        };
        let Some(open_paren) = source[at..].find('(').map(|n| at + n) else {
            continue;
        };
        let Some(close_paren) = source[open_paren..].find(')').map(|n| open_paren + n) else {
            continue;
        };
        let vars: Vec<String> = source[open_paren + 1..close_paren]
            .split(',')
            .filter_map(|p| p.split('$').nth(1))
            .map(|v| {
                v.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or("")
                    .to_owned()
            })
            .collect();
        let Some(open) = source[close_paren..].find('{').map(|n| close_paren + n) else {
            continue;
        };
        let Some(close) = matching_brace(source, open) else {
            continue;
        };
        let Some((query, params)) = vars.split_first() else {
            continue;
        };
        out.push(Scope {
            name,
            query: query.clone(),
            params: params.to_vec(),
            body: source[open + 1..close].to_owned(),
        });
        at = close;
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub sql: String,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Var,
    Ident,
    Str,
    Num,
    Sym,
}

#[derive(Clone, Debug)]
struct Tok {
    kind: Kind,
    text: String,
    start: usize,
    end: usize,
}

fn tokenize(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if src[i..].starts_with("<?php") {
            i += 5;
            continue;
        }
        if src[i..].starts_with("//") || (c == b'#' && b.get(i + 1) != Some(&b'[')) {
            i = src[i..].find('\n').map_or(b.len(), |n| i + n);
            continue;
        }
        if src[i..].starts_with("/*") {
            i = src[i + 2..].find("*/").map_or(b.len(), |n| i + n + 4);
            continue;
        }
        let start = i;
        if c == b'\'' || c == b'"' {
            let mut text = String::new();
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                if b[j] == b'\\' && j + 1 < b.len() {
                    let next = b[j + 1];
                    if next == c || next == b'\\' {
                        text.push(next as char);
                        j += 2;
                        continue;
                    }
                }
                let ch = src[j..].chars().next().unwrap_or(' ');
                text.push(ch);
                j += ch.len_utf8();
            }
            i = (j + 1).min(b.len());
            out.push(Tok {
                kind: Kind::Str,
                text,
                start,
                end: i,
            });
            continue;
        }
        if c == b'$'
            && b.get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == b'_')
        {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            out.push(Tok {
                kind: Kind::Var,
                text: src[i + 1..j].to_owned(),
                start,
                end: j,
            });
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.' || b[j] == b'_') {
                j += 1;
            }
            out.push(Tok {
                kind: Kind::Num,
                text: src[i..j].replace('_', ""),
                start,
                end: j,
            });
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'\\' {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'\\') {
                j += 1;
            }
            out.push(Tok {
                kind: Kind::Ident,
                text: src[i..j].to_owned(),
                start,
                end: j,
            });
            i = j;
            continue;
        }
        let sym = [
            "?->", "->", "::", "=>", "===", "!==", "==", "!=", "<=", ">=", "<>",
        ]
        .into_iter()
        .find(|s| src[i..].starts_with(s))
        .unwrap_or_else(|| &src[i..i + src[i..].chars().next().map_or(1, char::len_utf8)]);
        i += sym.len();
        out.push(Tok {
            kind: Kind::Sym,
            text: sym.to_owned(),
            start,
            end: i,
        });
    }
    out
}

#[derive(Clone, Debug)]
enum Val {
    Str(String),
    Num(String),
    Bool(bool),
    Null,
    Array(Vec<(Option<Val>, Val)>),
    Raw(String),
    Now,
    Closure(Vec<Call>),
    Other(String),
}

#[derive(Clone, Debug)]
struct Call {
    name: String,
    args: Vec<Val>,
}

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Tok>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            toks: tokenize(src),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n)
    }

    fn is_sym(&self, s: &str) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == Kind::Sym && t.text == s)
    }

    fn is_ident(&self, s: &str) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == Kind::Ident && t.text.eq_ignore_ascii_case(s))
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        let yes = self.is_sym(s);
        if yes {
            self.pos += 1;
        }
        yes
    }

    fn expect_sym(&mut self, s: &str) -> Result<(), String> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(match self.peek() {
                Some(t) => format!("Expected `{s}` before `{}`", t.text),
                None => format!("Expected `{s}` at the end"),
            })
        }
    }

    fn skip_balanced(&mut self) {
        let mut depth = 0i32;
        while let Some(t) = self.peek() {
            if t.kind == Kind::Sym {
                match t.text.as_str() {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" if depth == 0 => return,
                    ")" | "]" | "}" => depth -= 1,
                    "," | "=>" if depth == 0 => return,
                    _ => {}
                }
            }
            self.pos += 1;
        }
    }

    fn other_from(&mut self, start_tok: usize) -> Val {
        self.pos = start_tok;
        self.skip_balanced();
        let start = self.toks[start_tok].start;
        let end = self
            .toks
            .get(self.pos.saturating_sub(1))
            .map_or(start, |t| t.end)
            .max(start);
        Val::Other(self.src[start..end].trim().to_owned())
    }

    fn args(&mut self) -> Result<Vec<Val>, String> {
        self.expect_sym("(")?;
        let mut out = Vec::new();
        while !self.is_sym(")") {
            if self.peek().is_none() {
                return Err("Unclosed `(`".into());
            }
            if self.peek().is_some_and(|t| t.kind == Kind::Ident)
                && self
                    .peek_at(1)
                    .is_some_and(|t| t.kind == Kind::Sym && t.text == ":")
            {
                self.pos += 2;
            }
            out.push(self.value()?);
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(out)
    }

    fn array(&mut self, close: &str) -> Result<Val, String> {
        let mut items = Vec::new();
        while !self.is_sym(close) {
            if self.peek().is_none() {
                return Err("Unclosed array".into());
            }
            let first = self.value()?;
            if self.eat_sym("=>") {
                let v = self.value()?;
                items.push((Some(first), v));
            } else {
                items.push((None, first));
            }
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(close)?;
        Ok(Val::Array(items))
    }

    fn value(&mut self) -> Result<Val, String> {
        let start = self.pos;
        let Some(t) = self.peek().cloned() else {
            return Err("Expected a value".into());
        };
        let v = match t.kind {
            Kind::Str => {
                self.pos += 1;
                Val::Str(t.text)
            }
            Kind::Num => {
                self.pos += 1;
                Val::Num(t.text)
            }
            Kind::Sym if t.text == "-" && self.peek_at(1).is_some_and(|n| n.kind == Kind::Num) => {
                let n = self.peek_at(1).map(|n| n.text.clone()).unwrap_or_default();
                self.pos += 2;
                Val::Num(format!("-{n}"))
            }
            Kind::Sym if t.text == "[" => {
                self.pos += 1;
                self.array("]")?
            }
            Kind::Ident
                if t.text.eq_ignore_ascii_case("array")
                    && self.peek_at(1).is_some_and(|n| n.text == "(") =>
            {
                self.pos += 2;
                self.array(")")?
            }
            Kind::Ident if t.text.eq_ignore_ascii_case("true") => {
                self.pos += 1;
                Val::Bool(true)
            }
            Kind::Ident if t.text.eq_ignore_ascii_case("false") => {
                self.pos += 1;
                Val::Bool(false)
            }
            Kind::Ident if t.text.eq_ignore_ascii_case("null") => {
                self.pos += 1;
                Val::Null
            }
            Kind::Ident
                if t.text.eq_ignore_ascii_case("static")
                    && self.peek_at(1).is_some_and(|n| n.kind == Kind::Ident) =>
            {
                self.pos += 1;
                return self.value();
            }
            Kind::Ident if t.text == "fn" || t.text == "function" => self.closure()?,
            Kind::Ident
                if is_db(&t.text)
                    && self.peek_at(1).is_some_and(|n| n.text == "::")
                    && self.peek_at(2).is_some_and(|n| n.text == "raw") =>
            {
                self.pos += 3;
                let args = self.args()?;
                match args.into_iter().next() {
                    Some(Val::Str(s)) => Val::Raw(s),
                    _ => return Ok(self.other_from(start)),
                }
            }
            Kind::Ident
                if (t.text == "now" || t.text == "\\now")
                    && self.peek_at(1).is_some_and(|n| n.text == "(")
                    && self.peek_at(2).is_some_and(|n| n.text == ")") =>
            {
                self.pos += 3;
                Val::Now
            }
            _ => return Ok(self.other_from(start)),
        };
        if self.peek().is_some_and(|n| {
            n.kind == Kind::Sym && !matches!(n.text.as_str(), "," | ")" | "]" | "=>")
        }) {
            return Ok(self.other_from(start));
        }
        Ok(v)
    }

    fn closure(&mut self) -> Result<Val, String> {
        let arrow = self.peek().is_some_and(|t| t.text == "fn");
        self.pos += 1;
        self.expect_sym("(")?;
        let mut param = None;
        while !self.is_sym(")") {
            let Some(t) = self.peek().cloned() else {
                return Err("Unclosed closure parameters".into());
            };
            if t.kind == Kind::Var && param.is_none() {
                param = Some(t.text);
            }
            self.pos += 1;
        }
        self.pos += 1;
        if self.is_ident("use") {
            self.pos += 1;
            self.args()?;
        }
        if self.eat_sym(":") {
            self.pos += 1;
        }
        let param = param.unwrap_or_default();
        if arrow {
            self.expect_sym("=>")?;
            let calls = self.var_chain(&param)?;
            return Ok(Val::Closure(calls));
        }
        self.expect_sym("{")?;
        let mut calls = Vec::new();
        let mut depth = 0;
        while let Some(t) = self.peek().cloned() {
            if t.kind == Kind::Sym && t.text == "}" && depth == 0 {
                self.pos += 1;
                break;
            }
            if t.kind == Kind::Sym && t.text == "{" {
                depth += 1;
            }
            if t.kind == Kind::Sym && t.text == "}" {
                depth -= 1;
            }
            if t.kind == Kind::Var
                && t.text == param
                && self
                    .peek_at(1)
                    .is_some_and(|n| n.text == "->" || n.text == "?->")
            {
                calls.extend(self.var_chain(&param)?);
                continue;
            }
            self.pos += 1;
        }
        Ok(Val::Closure(calls))
    }

    fn var_chain(&mut self, param: &str) -> Result<Vec<Call>, String> {
        if self
            .peek()
            .is_some_and(|t| t.kind == Kind::Var && t.text == param)
        {
            self.pos += 1;
        } else {
            return Err("A closure must build on its query parameter".into());
        }
        self.calls()
    }

    fn calls(&mut self) -> Result<Vec<Call>, String> {
        let mut out = Vec::new();
        while self.eat_sym("->") || self.eat_sym("?->") {
            let Some(t) = self.peek().cloned() else {
                return Err("Expected a method name".into());
            };
            if t.kind != Kind::Ident {
                return Err(format!("Expected a method name, found `{}`", t.text));
            }
            self.pos += 1;
            if !self.is_sym("(") {
                return Err(format!(
                    "`{}` is a property: only method calls can be translated",
                    t.text
                ));
            }
            let args = self.args()?;
            out.push(Call { name: t.text, args });
        }
        Ok(out)
    }
}

fn is_db(name: &str) -> bool {
    matches!(
        name.trim_start_matches('\\'),
        "DB" | "Illuminate\\Support\\Facades\\DB"
    )
}

fn basename(class: &str) -> &str {
    class.rsplit('\\').next().unwrap_or(class)
}

pub fn guess_table(class: &str) -> String {
    let mut snake = String::new();
    for (i, c) in basename(class).chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }
    let (head, last) = snake
        .rsplit_once('_')
        .map_or(("", snake.as_str()), |(h, l)| (h, l));
    let plural = match last {
        "person" => "people".to_owned(),
        "child" => "children".to_owned(),
        l if l.ends_with('y')
            && !l.ends_with("ay")
            && !l.ends_with("ey")
            && !l.ends_with("oy")
            && !l.ends_with("uy") =>
        {
            format!("{}ies", &l[..l.len() - 1])
        }
        l if l.ends_with('s')
            || l.ends_with('x')
            || l.ends_with('z')
            || l.ends_with("ch")
            || l.ends_with("sh") =>
        {
            format!("{l}es")
        }
        l => format!("{l}s"),
    };
    if head.is_empty() {
        plural
    } else {
        format!("{head}_{plural}")
    }
}

fn quote_part(p: &str) -> String {
    let p = p.trim().trim_matches('`');
    if p == "*" {
        "*".to_owned()
    } else {
        format!("`{p}`")
    }
}

fn ident(name: &str) -> String {
    let name = name.trim();
    let lower = name.to_ascii_lowercase();
    if let Some(i) = lower.find(" as ") {
        return format!("{} AS {}", ident(&name[..i]), quote_part(&name[i + 4..]));
    }
    if name.contains(['(', ' ', '+', '-', '*', '/']) && name != "*" && !name.ends_with(".*") {
        return name.to_owned();
    }
    name.split('.')
        .map(quote_part)
        .collect::<Vec<_>>()
        .join(".")
}

fn table_ref(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if let Some(i) = lower.find(" as ") {
        return format!(
            "{} AS {}",
            quote_part(&name[..i]),
            quote_part(&name[i + 4..])
        );
    }
    ident(name)
}

fn str_lit(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
}

#[derive(Clone, Copy, PartialEq)]
enum Soft {
    Exclude,
    With,
    Only,
}

enum Action {
    Select,
    Aggregate(String),
    Exists(bool),
    Delete,
    Update(Vec<(String, String)>),
    Insert(
        &'static str,
        Vec<String>,
        Vec<Vec<String>>,
        Option<Vec<String>>,
    ),
    Truncate,
}

struct Query {
    table: String,
    base: String,
    eloquent: bool,
    soft: Option<Soft>,
    distinct: bool,
    columns: Vec<String>,
    joins: Vec<String>,
    wheres: Vec<(bool, String)>,
    groups: Vec<String>,
    havings: Vec<(bool, String)>,
    orders: Vec<String>,
    limit: Option<String>,
    offset: Option<String>,
    lock: Option<&'static str>,
    action: Action,
    notes: Vec<String>,
    bindings: Vec<String>,
    scopes: Vec<Scope>,
}

fn php_text(v: &Val) -> Option<String> {
    Some(match v {
        Val::Str(s) => format!("'{s}'"),
        Val::Num(n) => n.clone(),
        Val::Bool(b) => b.to_string(),
        Val::Null => "null".to_owned(),
        Val::Other(t) => t.clone(),
        _ => return None,
    })
}

fn substitute(calls: &[Call], names: &[String], args: &[Val]) -> Vec<Call> {
    let replace = |v: &Val| -> Val {
        match v {
            Val::Other(text) => {
                if let Some(i) = names.iter().position(|n| text == &format!("${n}"))
                    && let Some(a) = args.get(i)
                {
                    return a.clone();
                }
                let mut t = text.clone();
                for (n, a) in names.iter().zip(args) {
                    if let Some(lit) = php_text(a) {
                        t = t.replace(&format!("${n}"), &lit);
                    }
                }
                Val::Other(t)
            }
            Val::Closure(inner) => Val::Closure(substitute(inner, names, args)),
            Val::Array(items) => Val::Array(
                items
                    .iter()
                    .map(|(k, v)| (k.clone(), substitute_val(v, names, args)))
                    .collect(),
            ),
            other => other.clone(),
        }
    };
    calls
        .iter()
        .map(|c| Call {
            name: c.name.clone(),
            args: c.args.iter().map(replace).collect(),
        })
        .collect()
}

fn substitute_val(v: &Val, names: &[String], args: &[Val]) -> Val {
    let call = Call {
        name: String::new(),
        args: vec![v.clone()],
    };
    substitute(std::slice::from_ref(&call), names, args)
        .pop()
        .and_then(|c| c.args.into_iter().next())
        .unwrap_or_else(|| v.clone())
}

impl Query {
    fn new(table: &str, eloquent: bool, soft: bool) -> Self {
        Self {
            table: table_ref(table),
            base: table.split_whitespace().next().unwrap_or(table).to_owned(),
            eloquent,
            soft: soft.then_some(Soft::Exclude),
            distinct: false,
            columns: Vec::new(),
            joins: Vec::new(),
            wheres: Vec::new(),
            groups: Vec::new(),
            havings: Vec::new(),
            orders: Vec::new(),
            limit: None,
            offset: None,
            lock: None,
            action: Action::Select,
            notes: Vec::new(),
            bindings: Vec::new(),
            scopes: Vec::new(),
        }
    }

    fn nested(&self) -> Self {
        let mut q = Self::new(&self.base, self.eloquent, false);
        q.scopes = self.scopes.clone();
        q
    }

    fn scope(&mut self, call: &Call) -> bool {
        let Some(scope) = self.scopes.iter().find(|s| s.name == call.name).cloned() else {
            return false;
        };
        let mut p = Parser::new(&scope.body);
        let mut calls = Vec::new();
        while let Some(t) = p.peek().cloned() {
            if t.kind == Kind::Var
                && t.text == scope.query
                && p.peek_at(1).is_some_and(|n| n.text == "->")
            {
                match p.var_chain(&scope.query) {
                    Ok(c) => calls.extend(c),
                    Err(_) => return false,
                }
                continue;
            }
            p.pos += 1;
        }
        if calls.is_empty() {
            return false;
        }
        for c in substitute(&calls, &scope.params, &call.args) {
            self.apply(&c);
        }
        true
    }

    fn note(&mut self, n: impl Into<String>) {
        let n = n.into();
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    fn val(&mut self, v: &Val) -> String {
        match v {
            Val::Str(s) => str_lit(s),
            Val::Num(n) => n.clone(),
            Val::Bool(b) => if *b { "1" } else { "0" }.to_owned(),
            Val::Null => "NULL".to_owned(),
            Val::Raw(r) => r.clone(),
            Val::Now => "NOW()".to_owned(),
            Val::Array(items) => {
                let parts: Vec<String> = items.iter().map(|(_, v)| self.val(v)).collect();
                format!("({})", parts.join(", "))
            }
            Val::Closure(_) => {
                self.note("A subquery closure isn't translated: it shows as ?");
                "?".to_owned()
            }
            Val::Other(text) => {
                self.bindings.push(text.clone());
                "?".to_owned()
            }
        }
    }

    fn col(&mut self, v: &Val) -> String {
        match v {
            Val::Str(s) => ident(s),
            Val::Raw(r) => r.clone(),
            other => self.val(other),
        }
    }

    fn qualified(&self, col: &str) -> String {
        format!("{}.{}", quote_part(&self.base), quote_part(col))
    }

    fn push_where(&mut self, or: bool, cond: String) {
        self.wheres.push((or, cond));
    }

    fn where_list(list: &[(bool, String)]) -> String {
        let mut out = String::new();
        for (i, (or, cond)) in list.iter().enumerate() {
            if i > 0 {
                out.push_str(if *or { " OR " } else { " AND " });
            }
            out.push_str(cond);
        }
        out
    }

    fn group(&mut self, calls: &[Call]) -> Option<String> {
        let mut sub = self.nested();
        for c in calls {
            sub.apply(c);
        }
        for n in sub.notes {
            self.note(n);
        }
        self.bindings.extend(sub.bindings);
        (!sub.wheres.is_empty()).then(|| format!("({})", Self::where_list(&sub.wheres)))
    }

    fn comparison(&mut self, args: &[Val]) -> Option<String> {
        match args {
            [Val::Closure(calls)] => self.group(calls),
            [Val::Array(items)] => {
                let mut parts = Vec::new();
                for (k, v) in items {
                    match (k, v) {
                        (Some(k), v) => {
                            let c = self.col(k);
                            parts.push(self.compare(&c, "=", v));
                        }
                        (None, Val::Array(inner)) => {
                            let vals: Vec<Val> = inner.iter().map(|(_, v)| v.clone()).collect();
                            if let Some(p) = self.comparison(&vals) {
                                parts.push(p);
                            }
                        }
                        _ => {}
                    }
                }
                (!parts.is_empty()).then(|| parts.join(" AND "))
            }
            [col, value] => {
                let c = self.col(col);
                Some(self.compare(&c, "=", value))
            }
            [col, Val::Str(op), value, ..] => {
                let c = self.col(col);
                Some(self.compare(&c, op, value))
            }
            _ => None,
        }
    }

    fn compare(&mut self, col: &str, op: &str, value: &Val) -> String {
        let op = op.trim().to_ascii_uppercase();
        match (value, op.as_str()) {
            (Val::Null, "=" | "IS") => format!("{col} IS NULL"),
            (Val::Null, "!=" | "<>" | "IS NOT") => format!("{col} IS NOT NULL"),
            _ => format!("{col} {op} {}", self.val(value)),
        }
    }

    fn apply(&mut self, call: &Call) {
        let (or, name) = match call.name.strip_prefix("or") {
            Some(rest) if rest.starts_with(|c: char| c.is_ascii_uppercase()) => {
                let mut n = rest.to_owned();
                n[..1].make_ascii_lowercase();
                (true, n)
            }
            _ => (false, call.name.clone()),
        };
        let a = &call.args;
        match name.as_str() {
            "query"
            | "newQuery"
            | "toBase"
            | "toSql"
            | "toRawSql"
            | "dd"
            | "dump"
            | "ddRawSql"
            | "dumpRawSql"
            | "clone"
            | "tap"
            | "withoutGlobalScopes"
            | "useWritePdo" => {}
            "where" | "having" if name == "having" => {
                if let Some(c) = self.comparison(a) {
                    self.havings.push((or, c));
                }
            }
            "where" => {
                if let Some(c) = self.comparison(a) {
                    self.push_where(or, c);
                }
            }
            "whereNot" => {
                if let Some(c) = self.comparison(a) {
                    let c = if c.starts_with('(') {
                        c
                    } else {
                        format!("({c})")
                    };
                    self.push_where(or, format!("NOT {c}"));
                }
            }
            "whereIn" | "whereNotIn" | "whereIntegerInRaw" | "whereIntegerNotInRaw" => {
                if let [col, values, ..] = a.as_slice() {
                    let c = self.col(col);
                    let not = if name.contains("Not") { "NOT IN" } else { "IN" };
                    let list = match values {
                        Val::Array(items) if items.is_empty() => {
                            self.push_where(
                                or,
                                if not == "IN" { "0 = 1" } else { "1 = 1" }.to_owned(),
                            );
                            return;
                        }
                        Val::Array(_) => self.val(values),
                        other => format!("({})", self.val(other)),
                    };
                    self.push_where(or, format!("{c} {not} {list}"));
                }
            }
            "whereNull" | "whereNotNull" => {
                let not = if name == "whereNotNull" {
                    "IS NOT NULL"
                } else {
                    "IS NULL"
                };
                let cols: Vec<Val> = match a.first() {
                    Some(Val::Array(items)) => items.iter().map(|(_, v)| v.clone()).collect(),
                    Some(v) => vec![v.clone()],
                    None => Vec::new(),
                };
                for (i, col) in cols.iter().enumerate() {
                    let c = self.col(col);
                    self.push_where(or && i == 0, format!("{c} {not}"));
                }
            }
            "whereBetween" | "whereNotBetween" | "havingBetween" => {
                if let [col, Val::Array(range), ..] = a.as_slice()
                    && range.len() == 2
                {
                    let c = self.col(col);
                    let (lo, hi) = (self.val(&range[0].1), self.val(&range[1].1));
                    let not = if name == "whereNotBetween" {
                        "NOT BETWEEN"
                    } else {
                        "BETWEEN"
                    };
                    let cond = format!("{c} {not} {lo} AND {hi}");
                    if name == "havingBetween" {
                        self.havings.push((or, cond));
                    } else {
                        self.push_where(or, cond);
                    }
                }
            }
            "whereColumn" => match a.as_slice() {
                [x, y] => {
                    let (x, y) = (self.col(x), self.col(y));
                    self.push_where(or, format!("{x} = {y}"));
                }
                [x, Val::Str(op), y, ..] => {
                    let (x, y) = (self.col(x), self.col(y));
                    self.push_where(or, format!("{x} {op} {y}"));
                }
                _ => {}
            },
            "whereRaw" | "havingRaw" => {
                if let Some(Val::Str(sql)) = a.first() {
                    let bound = self.bind_raw(sql, a.get(1));
                    if name == "havingRaw" {
                        self.havings.push((or, bound));
                    } else {
                        self.push_where(or, bound);
                    }
                }
            }
            "whereDate" | "whereMonth" | "whereDay" | "whereYear" | "whereTime" => {
                let f = &name[5..].to_ascii_uppercase();
                let (col, op, value) = match a.as_slice() {
                    [c, v] => (c, "=".to_owned(), v),
                    [c, Val::Str(op), v, ..] => (c, op.clone(), v),
                    _ => return,
                };
                let c = self.col(col);
                let cond = self.compare(&format!("{f}({c})"), &op, value);
                self.push_where(or, cond);
            }
            "whereLike" | "whereNotLike" => {
                if let [col, value, ..] = a.as_slice() {
                    let c = self.col(col);
                    let op = if name == "whereNotLike" {
                        "NOT LIKE"
                    } else {
                        "LIKE"
                    };
                    let cond = self.compare(&c, op, value);
                    self.push_where(or, cond);
                }
            }
            "whereKey" | "whereKeyNot" => {
                if let Some(v) = a.first() {
                    let c = self.qualified("id");
                    let cond = match (v, name.as_str()) {
                        (Val::Array(_), "whereKey") => format!("{c} IN {}", self.val(v)),
                        (Val::Array(_), _) => format!("{c} NOT IN {}", self.val(v)),
                        (_, "whereKey") => self.compare(&c, "=", v),
                        _ => self.compare(&c, "!=", v),
                    };
                    self.push_where(or, cond);
                }
            }
            "whereHas" | "whereDoesntHave" | "has" | "doesntHave" | "whereRelation"
            | "whereBelongsTo" | "whereMorphedTo" | "withWhereHas" | "whereExists"
            | "whereNotExists" => {
                self.note(format!(
                    "{}() needs the relation's query and isn't translated",
                    call.name
                ));
            }
            "select" => {
                self.columns.clear();
                self.add_columns(a);
            }
            "addSelect" => self.add_columns(a),
            "selectRaw" => {
                if let Some(Val::Str(sql)) = a.first() {
                    let bound = self.bind_raw(sql, a.get(1));
                    self.columns.push(bound);
                }
            }
            "distinct" => self.distinct = true,
            "from" | "table" => {
                if let Some(Val::Str(t)) = a.first() {
                    self.table = table_ref(t);
                    self.base = t.split_whitespace().next().unwrap_or(t).to_owned();
                }
            }
            "join" | "leftJoin" | "rightJoin" | "crossJoin" | "joinWhere" | "leftJoinWhere" => {
                let kind = match name.as_str() {
                    "leftJoin" | "leftJoinWhere" => "LEFT JOIN",
                    "rightJoin" => "RIGHT JOIN",
                    "crossJoin" => "CROSS JOIN",
                    _ => "INNER JOIN",
                };
                match a.as_slice() {
                    [Val::Str(t)] => self.joins.push(format!("{kind} {}", table_ref(t))),
                    [Val::Str(t), x, y] => {
                        let (x, y) = (self.col(x), self.col(y));
                        self.joins
                            .push(format!("{kind} {} ON {x} = {y}", table_ref(t)));
                    }
                    [Val::Str(t), x, Val::Str(op), y, ..] => {
                        let (x, y) = (self.col(x), self.col(y));
                        self.joins
                            .push(format!("{kind} {} ON {x} {op} {y}", table_ref(t)));
                    }
                    [Val::Str(t), Val::Closure(_)] => {
                        self.joins.push(format!("{kind} {} ON ?", table_ref(t)));
                        self.note("A join closure isn't translated: its ON shows as ?");
                    }
                    _ => self.note(format!(
                        "{}() with these arguments isn't translated",
                        call.name
                    )),
                }
            }
            "groupBy" => {
                for v in a {
                    match v {
                        Val::Array(items) => {
                            for (_, v) in items {
                                let c = self.col(v);
                                self.groups.push(c);
                            }
                        }
                        v => {
                            let c = self.col(v);
                            self.groups.push(c);
                        }
                    }
                }
            }
            "groupByRaw" | "orderByRaw" => {
                if let Some(Val::Str(sql)) = a.first() {
                    let bound = self.bind_raw(sql, a.get(1));
                    if name == "groupByRaw" {
                        self.groups.push(bound);
                    } else {
                        self.orders.push(bound);
                    }
                }
            }
            "orderBy" | "orderByDesc" | "latest" | "oldest" | "reorder" => {
                if name == "reorder" {
                    self.orders.clear();
                    if a.is_empty() {
                        return;
                    }
                }
                let default = if matches!(name.as_str(), "latest" | "oldest") {
                    "created_at"
                } else {
                    ""
                };
                let c = match a.first() {
                    Some(v) => self.col(v),
                    None if !default.is_empty() => ident(default),
                    None => return,
                };
                let desc = match name.as_str() {
                    "orderByDesc" | "latest" => true,
                    "oldest" => false,
                    _ => matches!(a.get(1), Some(Val::Str(d)) if d.eq_ignore_ascii_case("desc")),
                };
                self.orders
                    .push(format!("{c} {}", if desc { "DESC" } else { "ASC" }));
            }
            "inRandomOrder" => self.orders.push("RAND()".to_owned()),
            "limit" | "take" => self.limit = a.first().map(|v| self.val(v)),
            "offset" | "skip" => self.offset = a.first().map(|v| self.val(v)),
            "forPage" => {
                if let [Val::Num(page), Val::Num(per), ..] = a.as_slice()
                    && let (Ok(p), Ok(n)) = (page.parse::<u64>(), per.parse::<u64>())
                {
                    self.limit = Some(n.to_string());
                    self.offset = Some((p.saturating_sub(1) * n).to_string());
                }
            }
            "lockForUpdate" => self.lock = Some("FOR UPDATE"),
            "sharedLock" => self.lock = Some("LOCK IN SHARE MODE"),
            "withTrashed" => {
                if self.soft.is_some() {
                    self.soft = Some(Soft::With);
                }
            }
            "onlyTrashed" => {
                if self.soft.is_some() {
                    self.soft = Some(Soft::Only);
                }
            }
            "withoutTrashed" => {
                if self.soft.is_some() {
                    self.soft = Some(Soft::Exclude);
                }
            }
            "with" | "without" | "load" | "withOnly" | "loadMissing" => {
                self.note("Eager loading runs as separate queries and isn't shown");
            }
            "withCount" | "withSum" | "withAvg" | "withMin" | "withMax" | "withExists"
            | "withAggregate" => {
                self.note(format!(
                    "{}() adds a subquery per relation and isn't translated",
                    call.name
                ));
            }
            "when" | "unless" => {
                if let Some(Val::Closure(calls)) = a.get(1) {
                    for c in calls {
                        self.apply(c);
                    }
                    self.note(format!("{}() is shown as if its callback runs", call.name));
                }
            }
            "get" | "all" | "cursor" | "lazy" | "lazyById" | "each" | "chunk" | "chunkById"
            | "first" | "firstOrFail" | "sole" | "paginate" | "simplePaginate"
            | "cursorPaginate" => {
                if let Some(Val::Array(items)) = a.first() {
                    self.columns.clear();
                    self.add_columns(&items.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>());
                }
                match name.as_str() {
                    "first" | "firstOrFail" | "sole" => self.limit = Some("1".to_owned()),
                    "chunk" | "chunkById" | "lazy" | "lazyById" => {
                        if let Some(Val::Num(n)) = a.first() {
                            self.limit = Some(n.clone());
                        }
                        self.note(format!("{}() repeats the query page by page", call.name));
                    }
                    "paginate" | "simplePaginate" | "cursorPaginate" => {
                        let per = match a.first() {
                            Some(Val::Num(n)) => n.clone(),
                            _ => "15".to_owned(),
                        };
                        self.limit = Some(per);
                        self.offset = Some("0".to_owned());
                        if name == "paginate" {
                            self.note(
                                "paginate() also runs a count query; the first page is shown",
                            );
                        } else {
                            self.note("The first page is shown");
                        }
                    }
                    _ => {}
                }
            }
            "find" | "findOrFail" | "findMany" | "findOr" => {
                let key = self.qualified("id");
                match a.first() {
                    Some(v @ Val::Array(_)) => {
                        let list = self.val(v);
                        self.push_where(false, format!("{key} IN {list}"));
                    }
                    Some(v) => {
                        let cond = self.compare(&key, "=", v);
                        self.push_where(false, cond);
                        self.limit = Some("1".to_owned());
                    }
                    None => {}
                }
                if let Some(Val::Array(items)) = a.get(1) {
                    self.columns.clear();
                    self.add_columns(&items.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>());
                }
            }
            "value" => {
                if let Some(v) = a.first() {
                    let c = self.col(v);
                    self.columns = vec![c];
                    self.limit = Some("1".to_owned());
                }
            }
            "pluck" => {
                let cols: Vec<String> = a.iter().take(2).map(|v| self.col(v)).collect();
                self.columns = cols;
            }
            "count" | "sum" | "avg" | "average" | "min" | "max" => {
                let f = match name.as_str() {
                    "average" => "AVG".to_owned(),
                    n => n.to_ascii_uppercase(),
                };
                let c = match a.first() {
                    Some(v) => self.col(v),
                    None => "*".to_owned(),
                };
                self.action = Action::Aggregate(format!("{f}({c}) AS `aggregate`"));
            }
            "exists" => self.action = Action::Exists(true),
            "doesntExist" => self.action = Action::Exists(false),
            "delete" | "forceDelete" => {
                if self.soft.is_some() && name == "delete" {
                    let sets = vec![
                        (ident("deleted_at"), "NOW()".to_owned()),
                        (ident("updated_at"), "NOW()".to_owned()),
                    ];
                    self.action = Action::Update(sets);
                    self.note("The model uses soft deletes: delete() sets deleted_at");
                } else {
                    self.action = Action::Delete;
                }
            }
            "update" => {
                if let Some(Val::Array(items)) = a.first() {
                    let mut sets = Vec::new();
                    for (k, v) in items {
                        if let Some(k) = k {
                            let c = self.col(k);
                            let v = self.val(v);
                            sets.push((c, v));
                        }
                    }
                    if self.eloquent && !sets.iter().any(|(c, _)| c == "`updated_at`") {
                        sets.push((ident("updated_at"), "NOW()".to_owned()));
                    }
                    self.action = Action::Update(sets);
                }
            }
            "increment" | "decrement" | "incrementEach" | "decrementEach" => {
                let sign = if name.starts_with("increment") {
                    "+"
                } else {
                    "-"
                };
                let mut sets = Vec::new();
                match a.as_slice() {
                    [Val::Array(items), ..] => {
                        for (k, v) in items {
                            if let Some(k) = k {
                                let c = self.col(k);
                                let v = self.val(v);
                                sets.push((c.clone(), format!("{c} {sign} {v}")));
                            }
                        }
                    }
                    [col, rest @ ..] => {
                        let c = self.col(col);
                        let by = match rest.first() {
                            Some(v) => self.val(v),
                            None => "1".to_owned(),
                        };
                        sets.push((c.clone(), format!("{c} {sign} {by}")));
                        if let Some(Val::Array(extra)) = rest.get(1) {
                            for (k, v) in extra {
                                if let Some(k) = k {
                                    let c = self.col(k);
                                    let v = self.val(v);
                                    sets.push((c, v));
                                }
                            }
                        }
                    }
                    [] => {}
                }
                if self.eloquent {
                    sets.push((ident("updated_at"), "NOW()".to_owned()));
                }
                self.action = Action::Update(sets);
            }
            "insert" | "insertOrIgnore" | "insertGetId" | "create" | "forceCreate" | "upsert" => {
                let rows = match a.first() {
                    Some(Val::Array(items))
                        if items
                            .iter()
                            .all(|(k, v)| k.is_none() && matches!(v, Val::Array(_)))
                            && !items.is_empty() =>
                    {
                        items.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>()
                    }
                    Some(v @ Val::Array(_)) => vec![v.clone()],
                    _ => return,
                };
                let mut cols: Vec<String> = Vec::new();
                let mut values = Vec::new();
                for row in &rows {
                    let Val::Array(items) = row else { continue };
                    let mut vals = Vec::new();
                    for (k, v) in items {
                        let Some(k) = k else { continue };
                        let c = self.col(k);
                        if !cols.contains(&c) {
                            cols.push(c);
                        }
                        vals.push(self.val(v));
                    }
                    values.push(vals);
                }
                if matches!(name.as_str(), "create" | "forceCreate") {
                    for ts in ["updated_at", "created_at"] {
                        let c = ident(ts);
                        if !cols.contains(&c) {
                            cols.push(c);
                            for row in &mut values {
                                row.push("NOW()".to_owned());
                            }
                        }
                    }
                }
                let update = (name == "upsert").then(|| match a.get(2) {
                    Some(Val::Array(items)) => items.iter().map(|(_, v)| self.col(v)).collect(),
                    _ => cols.clone(),
                });
                let verb = if name == "insertOrIgnore" {
                    "INSERT IGNORE INTO"
                } else {
                    "INSERT INTO"
                };
                self.action = Action::Insert(verb, cols, values, update);
            }
            "truncate" => self.action = Action::Truncate,
            _ => {
                if !self.scope(call) {
                    self.note(format!(
                        "{}() is a scope or method that isn't translated",
                        call.name
                    ));
                }
            }
        }
    }

    fn add_columns(&mut self, args: &[Val]) {
        for v in args {
            match v {
                Val::Array(items) => {
                    for (k, v) in items {
                        match (k, v) {
                            (Some(Val::Str(alias)), Val::Closure(_)) => {
                                self.note("A subquery select isn't translated");
                                self.columns.push(format!("? AS {}", quote_part(alias)));
                            }
                            (_, v) => {
                                let c = self.col(v);
                                self.columns.push(c);
                            }
                        }
                    }
                }
                v => {
                    let c = self.col(v);
                    self.columns.push(c);
                }
            }
        }
    }

    fn bind_raw(&mut self, sql: &str, bindings: Option<&Val>) -> String {
        let values: Vec<String> = match bindings {
            Some(Val::Array(items)) => items.iter().map(|(_, v)| self.val(v)).collect(),
            _ => Vec::new(),
        };
        let mut out = String::new();
        let mut it = values.into_iter();
        for ch in sql.chars() {
            if ch == '?' {
                out.push_str(&it.next().unwrap_or_else(|| "?".to_owned()));
            } else {
                out.push(ch);
            }
        }
        out
    }

    fn where_sql(&self) -> Option<String> {
        let mut wheres = self.wheres.clone();
        match self.soft {
            Some(Soft::Exclude) | Some(Soft::Only) => {
                if wheres.iter().any(|(or, _)| *or) {
                    wheres = vec![(false, format!("({})", Self::where_list(&wheres)))];
                }
                let not = if self.soft == Some(Soft::Only) {
                    "IS NOT NULL"
                } else {
                    "IS NULL"
                };
                wheres.push((false, format!("{} {not}", self.qualified("deleted_at"))));
            }
            _ => {}
        }
        if wheres.is_empty() {
            return None;
        }
        let mut out = String::new();
        for (i, (or, cond)) in wheres.iter().enumerate() {
            if i > 0 {
                out.push_str(if *or { "\n  OR " } else { "\n  AND " });
            }
            out.push_str(cond);
        }
        Some(out)
    }

    fn select_sql(&self, head: &str) -> String {
        let mut sql = format!("{head}\nFROM {}", self.table);
        for j in &self.joins {
            sql.push('\n');
            sql.push_str(j);
        }
        if let Some(w) = self.where_sql() {
            sql.push_str("\nWHERE ");
            sql.push_str(&w);
        }
        if !self.groups.is_empty() {
            sql.push_str("\nGROUP BY ");
            sql.push_str(&self.groups.join(", "));
        }
        if !self.havings.is_empty() {
            sql.push_str("\nHAVING ");
            sql.push_str(&Self::where_list(&self.havings));
        }
        if !self.orders.is_empty() {
            sql.push_str("\nORDER BY ");
            sql.push_str(&self.orders.join(", "));
        }
        if let Some(l) = &self.limit {
            sql.push_str("\nLIMIT ");
            sql.push_str(l);
        }
        if let Some(o) = &self.offset {
            sql.push_str("\nOFFSET ");
            sql.push_str(o);
        }
        if let Some(lock) = self.lock {
            sql.push('\n');
            sql.push_str(lock);
        }
        sql
    }

    fn sql(&self) -> String {
        let cols = if self.columns.is_empty() {
            "*".to_owned()
        } else {
            self.columns.join(", ")
        };
        let distinct = if self.distinct { "DISTINCT " } else { "" };
        match &self.action {
            Action::Select => self.select_sql(&format!("SELECT {distinct}{cols}")),
            Action::Aggregate(expr) => self.select_sql(&format!("SELECT {expr}")),
            Action::Exists(yes) => {
                let inner = self
                    .select_sql(&format!("SELECT {cols}"))
                    .replace('\n', "\n  ");
                if *yes {
                    format!("SELECT EXISTS(\n  {inner}\n) AS `exists`")
                } else {
                    format!("SELECT NOT EXISTS(\n  {inner}\n) AS `exists`")
                }
            }
            Action::Delete => {
                let mut sql = if self.joins.is_empty() {
                    format!("DELETE FROM {}", self.table)
                } else {
                    format!(
                        "DELETE {} FROM {}\n{}",
                        quote_part(&self.base),
                        self.table,
                        self.joins.join("\n")
                    )
                };
                self.tail(&mut sql);
                sql
            }
            Action::Update(sets) => {
                let mut sql = format!("UPDATE {}", self.table);
                for j in &self.joins {
                    sql.push('\n');
                    sql.push_str(j);
                }
                let set: Vec<String> = sets.iter().map(|(c, v)| format!("{c} = {v}")).collect();
                sql.push_str("\nSET ");
                sql.push_str(&set.join(",\n    "));
                self.tail(&mut sql);
                sql
            }
            Action::Insert(verb, cols, rows, update) => {
                let rows: Vec<String> =
                    rows.iter().map(|r| format!("({})", r.join(", "))).collect();
                let mut sql = format!(
                    "{verb} {} ({})\nVALUES {}",
                    self.table,
                    cols.join(", "),
                    rows.join(",\n       ")
                );
                if let Some(update) = update {
                    let set: Vec<String> = update
                        .iter()
                        .map(|c| format!("{c} = VALUES({c})"))
                        .collect();
                    sql.push_str("\nON DUPLICATE KEY UPDATE ");
                    sql.push_str(&set.join(", "));
                }
                sql
            }
            Action::Truncate => format!("TRUNCATE TABLE {}", self.table),
        }
    }

    fn tail(&self, sql: &mut String) {
        if let Some(w) = self.where_sql() {
            sql.push_str("\nWHERE ");
            sql.push_str(&w);
        }
        if self.joins.is_empty() {
            if !self.orders.is_empty() {
                sql.push_str("\nORDER BY ");
                sql.push_str(&self.orders.join(", "));
            }
            if let Some(l) = &self.limit {
                sql.push_str("\nLIMIT ");
                sql.push_str(l);
            }
        }
    }
}

fn skip_prefix(p: &mut Parser<'_>) {
    loop {
        if p.is_ident("return") || p.is_ident("echo") || p.is_ident("yield") {
            p.pos += 1;
            continue;
        }
        if p.peek().is_some_and(|t| t.kind == Kind::Var)
            && p.peek_at(1)
                .is_some_and(|t| t.kind == Kind::Sym && t.text == "=")
        {
            p.pos += 2;
            continue;
        }
        break;
    }
}

pub fn to_sql(code: &str, model: &dyn Fn(&str) -> Option<Model>) -> Result<Output, String> {
    let mut p = Parser::new(code);
    skip_prefix(&mut p);
    let Some(head) = p.peek().cloned() else {
        return Err(
            "Write an Eloquent or query builder chain, like User::where('active', 1)->get()".into(),
        );
    };
    if head.kind == Kind::Var {
        return Err(format!(
            "Start the chain with DB::table('…') or a model (User::query()), not ${}",
            head.text
        ));
    }
    if head.kind != Kind::Ident || !p.peek_at(1).is_some_and(|t| t.text == "::") {
        return Err("Start the chain with DB::table('…') or a model, like User::query()".into());
    }
    p.pos += 2;
    let Some(method) = p.peek().cloned().filter(|t| t.kind == Kind::Ident) else {
        return Err("Expected a method after `::`".into());
    };
    p.pos += 1;
    let first_args = p.args()?;
    let mut calls = p.calls()?;
    p.eat_sym(";");
    if let Some(t) = p.peek() {
        return Err(format!("Only one query at a time: unexpected `{}`", t.text));
    }
    let mut query = if is_db(&head.text) {
        match (method.text.as_str(), first_args.first()) {
            ("table", Some(Val::Str(t))) => Query::new(t, false, false),
            ("connection", _) => {
                let Some(first) = calls.first().cloned() else {
                    return Err("Add ->table('…') after DB::connection()".into());
                };
                calls.remove(0);
                match (first.name.as_str(), first.args.first()) {
                    ("table", Some(Val::Str(t))) => Query::new(t, false, false),
                    _ => return Err("Add ->table('…') after DB::connection()".into()),
                }
            }
            (
                "select" | "statement" | "insert" | "update" | "delete" | "selectOne" | "scalar"
                | "affectingStatement",
                Some(Val::Str(sql)),
            ) => {
                let mut q = Query::new("", false, false);
                let bound = q.bind_raw(sql, first_args.get(1));
                return Ok(Output {
                    sql: bound.trim().to_owned(),
                    notes: binding_notes(&q),
                });
            }
            _ => {
                return Err(format!(
                    "DB::{}() isn't a query this can translate",
                    method.text
                ));
            }
        }
    } else {
        let info = model(&head.text);
        let mut q = match &info {
            Some(m) => {
                let mut q = Query::new(&m.table, true, m.soft_deletes);
                q.scopes = m.scopes.clone();
                q
            }
            None => {
                let t = guess_table(&head.text);
                let mut q = Query::new(&t, true, false);
                q.note(format!(
                    "{} isn't a known model here: the table name `{t}` is a guess",
                    basename(&head.text)
                ));
                q
            }
        };
        q.apply(&Call {
            name: method.text.clone(),
            args: first_args,
        });
        q
    };
    for c in &calls {
        query.apply(c);
    }
    let sql = query.sql();
    let notes = binding_notes(&query);
    Ok(Output { sql, notes })
}

fn binding_notes(q: &Query) -> Vec<String> {
    if q.bindings.is_empty() {
        return q.notes.clone();
    }
    let mut notes = q.notes.clone();
    notes.push(format!("? stands for {}", q.bindings.join(", ")));
    notes
}

pub fn statement_at(text: &str, at: usize) -> &str {
    let b = text.as_bytes();
    let mut depth = 0i32;
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\'' || c == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            i = j + 1;
            continue;
        }
        if text[i..].starts_with("//") || (c == b'#' && b.get(i + 1) != Some(&b'[')) {
            i = text[i..].find('\n').map_or(b.len(), |n| i + n);
            continue;
        }
        if text[i..].starts_with("/*") {
            i = text[i + 2..].find("*/").map_or(b.len(), |n| i + n + 4);
            continue;
        }
        match c {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b';' | b'{' | b'}' if depth <= 0 => {
                if i >= at && start <= at {
                    return text[start..i + usize::from(c == b';')].trim();
                }
                start = i + 1;
                depth = 0;
            }
            _ => {}
        }
        i += 1;
    }
    text[start.min(text.len())..].trim()
}

pub fn split_statements(code: &str) -> Vec<String> {
    let b = code.as_bytes();
    let mut depth = 0i32;
    let mut start = 0;
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\'' || c == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            i = j + 1;
            continue;
        }
        if code[i..].starts_with("//") || (c == b'#' && b.get(i + 1) != Some(&b'[')) {
            i = code[i..].find('\n').map_or(b.len(), |n| i + n);
            continue;
        }
        if code[i..].starts_with("/*") {
            i = code[i + 2..].find("*/").map_or(b.len(), |n| i + n + 4);
            continue;
        }
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth <= 0 => {
                out.push(code[start..=i].trim().to_owned());
                start = i + 1;
                depth = 0;
            }
            _ => {}
        }
        i += 1;
    }
    let rest = code.get(start..).unwrap_or_default().trim();
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out.retain(|s| !s.is_empty() && s != ";");
    out
}

pub fn tinker_script(code: &str) -> String {
    const KEEP: [&str; 30] = [
        "if",
        "else",
        "elseif",
        "foreach",
        "for",
        "while",
        "do",
        "switch",
        "try",
        "catch",
        "finally",
        "function",
        "class",
        "interface",
        "trait",
        "enum",
        "namespace",
        "use",
        "return",
        "echo",
        "print",
        "throw",
        "unset",
        "global",
        "static",
        "const",
        "abstract",
        "final",
        "readonly",
        "declare",
    ];
    let code = code.trim().trim_start_matches("<?php").trim();
    split_statements(code)
        .into_iter()
        .map(|s| {
            let body = s.trim_end_matches(';').trim();
            let first: String = body
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .to_ascii_lowercase();
            let keep = KEEP.contains(&first.as_str())
                || body.ends_with('}')
                || body.starts_with("dump(")
                || body.starts_with("dd(")
                || s.starts_with("//")
                || s.starts_with('#');
            if keep {
                if s.ends_with(';') || s.ends_with('}') {
                    s
                } else {
                    format!("{s};")
                }
            } else {
                format!("dump({body});")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn tinker_file(code: &str, source: &str, classes: &[String]) -> String {
    let mut imports: Vec<String> = split_statements(source.trim().trim_start_matches("<?php"))
        .into_iter()
        .filter(|s| s.starts_with("use ") && s.contains('\\'))
        .collect();
    let imported = |short: &str, imports: &[String]| {
        imports.iter().any(|u| {
            u.trim_end_matches(';')
                .rsplit(['\\', ' ', ',', '{'])
                .next()
                .is_some_and(|last| last.trim_end_matches('}') == short)
                || u.contains(&format!("\\{short},"))
                || u.contains(&format!("\\{short} "))
                || u.contains(&format!(" as {short}"))
        })
    };
    for class in classes {
        let short = basename(class);
        let used = [
            format!("{short}::"),
            format!("new {short}"),
            format!("{short} $"),
        ]
        .iter()
        .any(|p| code.contains(p.as_str()));
        if used && !imported(short, &imports) {
            imports.push(format!("use {};", class.trim_start_matches('\\')));
        }
    }
    let mut out = String::from("<?php\n\n");
    for u in &imports {
        out.push_str(u);
        out.push('\n');
    }
    if !imports.is_empty() {
        out.push('\n');
    }
    out.push_str(&tinker_script(code));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tinker_files_keep_the_sources_imports_and_add_models() {
        let source = "<?php\n\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Product;\nuse Illuminate\\Support\\Facades\\DB;\n\nclass A\n{\n    use Helpers;\n\n    public function index() { return Product::count(); }\n}\n";
        let classes = [
            "App\\Models\\Product".to_owned(),
            "App\\Models\\Category".to_owned(),
            "App\\Models\\User".to_owned(),
        ];
        assert_eq!(
            tinker_file("Product::count();\nCategory::first()", source, &classes),
            "<?php\n\nuse App\\Models\\Product;\nuse Illuminate\\Support\\Facades\\DB;\nuse App\\Models\\Category;\n\ndump(Product::count());\ndump(Category::first());\n"
        );
    }

    #[test]
    fn tinker_script_dumps_each_expression() {
        assert_eq!(
            tinker_script("<?php\n$users = User::where('a', 1)->get();\nUser::count()"),
            "dump($users = User::where('a', 1)->get());\ndump(User::count());"
        );
        assert_eq!(
            tinker_script("foreach (User::all() as $u) { echo $u->name; }\nreturn 1;"),
            "foreach (User::all() as $u) { echo $u->name; }\nreturn 1;"
        );
        assert_eq!(
            split_statements("a(';'); b(fn () => [1; 2]);"),
            ["a(';');", "b(fn () => [1; 2]);"]
        );
    }

    fn models(name: &str) -> Option<Model> {
        match name.rsplit('\\').next()? {
            "User" => Some(Model {
                table: "users".into(),
                soft_deletes: false,
                scopes: scopes_of(USER),
            }),
            "Post" => Some(Model {
                table: "posts".into(),
                soft_deletes: true,
                ..Model::default()
            }),
            _ => None,
        }
    }

    const USER: &str = "<?php\nclass User extends Model\n{\n    public function scopeActive(Builder $query): void\n    {\n        $query->where('active', 1)->whereNotNull('email_verified_at');\n    }\n\n    public function scopeOlderThan($query, int $age)\n    {\n        return $query->where('age', '>', $age);\n    }\n\n    #[Scope]\n    protected function cheaperThan(Builder $query, float $price): void\n    {\n        $query->where('price', '<', $price * 100);\n    }\n}\n";

    #[test]
    fn local_scopes_are_applied_from_the_model() {
        assert_eq!(
            sql("User::active()->olderThan(30)->get()"),
            "SELECT *\nFROM `users`\nWHERE `active` = 1\n  AND `email_verified_at` IS NOT NULL\n  AND `age` > 30"
        );
        let out = to_sql("User::cheaperThan(5)->get()", &models).unwrap();
        assert_eq!(out.sql, "SELECT *\nFROM `users`\nWHERE `price` < ?");
        assert_eq!(out.notes, ["? stands for 5 * 100"]);
        let names: Vec<String> = scopes_of(USER).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["active", "olderThan", "cheaperThan"]);
    }

    fn sql(code: &str) -> String {
        to_sql(code, &models).unwrap().sql
    }

    #[test]
    fn query_builder_select_with_joins_and_grouping() {
        let code = "$rows = DB::table('users as u')\n    ->select('u.id', 'u.name', DB::raw('COUNT(o.id) AS orders'))\n    ->leftJoin('orders as o', 'o.user_id', '=', 'u.id')\n    ->where('u.active', 1)\n    ->where('u.created_at', '>=', '2024-01-01')\n    ->groupBy('u.id', 'u.name')\n    ->orderByDesc('orders')\n    ->limit(10)\n    ->get();";
        assert_eq!(
            sql(code),
            "SELECT `u`.`id`, `u`.`name`, COUNT(o.id) AS orders\nFROM `users` AS `u`\nLEFT JOIN `orders` AS `o` ON `o`.`user_id` = `u`.`id`\nWHERE `u`.`active` = 1\n  AND `u`.`created_at` >= '2024-01-01'\nGROUP BY `u`.`id`, `u`.`name`\nORDER BY `orders` DESC\nLIMIT 10"
        );
    }

    #[test]
    fn eloquent_uses_the_models_table_and_terminal_methods() {
        assert_eq!(
            sql("User::where('email', 'a@b.c')->first();"),
            "SELECT *\nFROM `users`\nWHERE `email` = 'a@b.c'\nLIMIT 1"
        );
        assert_eq!(
            sql("User::find(5)"),
            "SELECT *\nFROM `users`\nWHERE `users`.`id` = 5\nLIMIT 1"
        );
        assert_eq!(
            sql("User::query()->count()"),
            "SELECT COUNT(*) AS `aggregate`\nFROM `users`"
        );
        assert_eq!(
            sql("User::latest()->pluck('name')"),
            "SELECT `name`\nFROM `users`\nORDER BY `created_at` DESC"
        );
    }

    #[test]
    fn closures_group_conditions_and_or_where() {
        let code = "User::where('active', true)->where(function ($q) { $q->where('role', 'admin')->orWhere('role', 'owner'); })->get()";
        assert_eq!(
            sql(code),
            "SELECT *\nFROM `users`\nWHERE `active` = 1\n  AND (`role` = 'admin' OR `role` = 'owner')"
        );
        let arrow =
            "User::where(fn ($q) => $q->whereNull('deleted_by')->orWhereIn('id', [1, 2]))->get()";
        assert_eq!(
            sql(arrow),
            "SELECT *\nFROM `users`\nWHERE (`deleted_by` IS NULL OR `id` IN (1, 2))"
        );
    }

    #[test]
    fn soft_deleted_models_filter_and_soft_delete() {
        assert_eq!(
            sql("Post::where('a', 1)->orWhere('b', 2)->get()"),
            "SELECT *\nFROM `posts`\nWHERE (`a` = 1 OR `b` = 2)\n  AND `posts`.`deleted_at` IS NULL"
        );
        assert_eq!(sql("Post::withTrashed()->get()"), "SELECT *\nFROM `posts`");
        assert_eq!(
            sql("Post::onlyTrashed()->count()"),
            "SELECT COUNT(*) AS `aggregate`\nFROM `posts`\nWHERE `posts`.`deleted_at` IS NOT NULL"
        );
        assert!(
            sql("Post::where('id', 1)->delete()")
                .starts_with("UPDATE `posts`\nSET `deleted_at` = NOW()")
        );
    }

    #[test]
    fn writes() {
        assert_eq!(
            sql(
                "DB::table('users')->where('id', 1)->update(['name' => 'Ana', 'votes' => DB::raw('votes + 1')])"
            ),
            "UPDATE `users`\nSET `name` = 'Ana',\n    `votes` = votes + 1\nWHERE `id` = 1"
        );
        assert_eq!(
            sql("User::where('id', 1)->increment('votes', 2)"),
            "UPDATE `users`\nSET `votes` = `votes` + 2,\n    `updated_at` = NOW()\nWHERE `id` = 1"
        );
        assert_eq!(
            sql(
                "DB::table('users')->insert([['email' => 'a@x', 'votes' => 0], ['email' => 'b@x', 'votes' => 1]])"
            ),
            "INSERT INTO `users` (`email`, `votes`)\nVALUES ('a@x', 0),\n       ('b@x', 1)"
        );
        assert_eq!(
            sql("DB::table('users')->where('votes', '>', 100)->delete()"),
            "DELETE FROM `users`\nWHERE `votes` > 100"
        );
        assert_eq!(
            sql("User::create(['name' => 'Ana'])"),
            "INSERT INTO `users` (`name`, `updated_at`, `created_at`)\nVALUES ('Ana', NOW(), NOW())"
        );
    }

    #[test]
    fn variables_become_placeholders_with_a_note() {
        let out = to_sql(
            "User::where('id', $id)->whereBetween('age', [$min, 30])->get()",
            &models,
        )
        .unwrap();
        assert_eq!(
            out.sql,
            "SELECT *\nFROM `users`\nWHERE `id` = ?\n  AND `age` BETWEEN ? AND 30"
        );
        assert!(
            out.notes.iter().any(|n| n == "? stands for $id, $min"),
            "{:?}",
            out.notes
        );
    }

    #[test]
    fn unknown_models_and_scopes_are_noted() {
        let out = to_sql("OrderItem::active()->get()", &models).unwrap();
        assert_eq!(out.sql, "SELECT *\nFROM `order_items`");
        assert_eq!(out.notes.len(), 2, "{:?}", out.notes);
        assert!(
            out.notes
                .iter()
                .any(|n| n.contains("table name `order_items` is a guess"))
        );
        assert!(out.notes.iter().any(|n| n.contains("active() is a scope")));
    }

    #[test]
    fn raw_parts_and_date_helpers() {
        assert_eq!(
            sql(
                "DB::table('orders')->selectRaw('SUM(total) AS t')->whereRaw('total > ?', [100])->whereDate('created_at', '2024-01-01')->get()"
            ),
            "SELECT SUM(total) AS t\nFROM `orders`\nWHERE total > 100\n  AND DATE(`created_at`) = '2024-01-01'"
        );
        assert_eq!(
            sql("DB::select('select * from users where id = ?', [1])"),
            "select * from users where id = 1"
        );
    }

    #[test]
    fn errors_explain_what_to_write() {
        assert!(
            to_sql("$user->posts()->get()", &models)
                .unwrap_err()
                .contains("Start the chain")
        );
        assert!(
            to_sql("User::where('a', 1)->name", &models)
                .unwrap_err()
                .contains("property")
        );
        assert!(to_sql("", &models).is_err());
    }

    #[test]
    fn guesses_table_names() {
        assert_eq!(guess_table("App\\Models\\Category"), "categories");
        assert_eq!(guess_table("OrderItem"), "order_items");
        assert_eq!(guess_table("Address"), "addresses");
        assert_eq!(guess_table("Person"), "people");
    }

    #[test]
    fn finds_the_statement_around_the_cursor() {
        let text = "<?php\nfunction a() {\n    $x = 1;\n    $users = User::where('a', 1)\n        ->where(function ($q) { $q->where('b', 2); })\n        ->get();\n    return $users;\n}\n";
        let at = text.find("->get").unwrap();
        assert_eq!(
            statement_at(text, at),
            "$users = User::where('a', 1)\n        ->where(function ($q) { $q->where('b', 2); })\n        ->get();"
        );
    }
}
