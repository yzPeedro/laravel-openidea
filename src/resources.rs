use crate::php::{Kind, Token, tokenize};
use crate::project::{Lines, find, rel};
use openidea_plugin::Project;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub args: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub line: u32,
    pub public: bool,
    pub is_static: bool,
    pub params: String,
    pub body: String,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Symbol {
    pub path: String,
    pub line: u32,
    pub keyword: String,
    pub name: String,
    pub namespace: String,
    pub is_abstract: bool,
    pub extends: Vec<String>,
    pub implements: Vec<String>,
    pub traits: Vec<String>,
    pub attributes: Vec<Attribute>,
    pub methods: Vec<Method>,
    pub backing: Option<String>,
    pub cases: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Call {
    pub path: String,
    pub line: u32,
    pub owner: String,
    pub class: String,
    pub method: String,
    pub args: Vec<String>,
    pub closure_types: Vec<String>,
}

#[derive(Default)]
pub struct Scan {
    pub symbols: Vec<Symbol>,
    pub calls: Vec<Call>,
}

pub fn short(name: &str) -> &str {
    name.trim_start_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or(name)
}

fn ident(tokens: &[Token], i: usize) -> Option<&str> {
    match &tokens.get(i)?.kind {
        Kind::Ident(s) => Some(s),
        _ => None,
    }
}

fn is(tokens: &[Token], i: usize, word: &str) -> bool {
    ident(tokens, i).is_some_and(|s| s.eq_ignore_ascii_case(word))
}

fn matching(tokens: &[Token], open: usize) -> usize {
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

fn start_of(text: &str, tokens: &[Token], k: usize) -> usize {
    tokens.get(k).map_or(text.len(), |t| t.start)
}

fn attributes(text: &str, tokens: &[Token], hash: usize) -> (Vec<Attribute>, usize) {
    let close = matching(tokens, hash + 1);
    let mut out = Vec::new();
    let mut j = hash + 2;
    while j < close {
        if let Some(name) = ident(tokens, j) {
            let mut args = String::new();
            let mut next = j + 1;
            if tokens.get(j + 1).is_some_and(|t| t.kind == Kind::Open('(')) {
                let end = matching(tokens, j + 1);
                args = text[tokens[j + 1].end..start_of(text, tokens, end)]
                    .trim()
                    .to_owned();
                next = end + 1;
            }
            out.push(Attribute {
                name: short(name).to_owned(),
                args,
            });
            j = next;
            while j < close && tokens[j].kind != Kind::Comma {
                j += 1;
            }
        }
        j += 1;
    }
    (out, close + 1)
}

fn names_until(
    tokens: &[Token],
    mut j: usize,
    stop: impl Fn(&Token) -> bool,
) -> (Vec<String>, usize) {
    let mut out = Vec::new();
    while j < tokens.len() && !stop(&tokens[j]) {
        if let Some(n) = ident(tokens, j)
            && !n.eq_ignore_ascii_case("implements")
            && !n.eq_ignore_ascii_case("extends")
        {
            out.push(short(n).to_owned());
        }
        j += 1;
    }
    (out, j)
}

pub fn class_refs(args: &str) -> Vec<String> {
    let tokens = tokenize(args, 0, args.len());
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind == Kind::DoubleColon
            && is(&tokens, i + 1, "class")
            && let Some(name) = i.checked_sub(1).and_then(|k| ident(&tokens, k))
        {
            out.push(short(name).to_owned());
        }
        if is(&tokens, i, "new")
            && let Some(name) = ident(&tokens, i + 1)
        {
            out.push(short(name).to_owned());
        }
    }
    out
}

pub fn param_types(params: &str) -> Vec<String> {
    let first = params.split(',').next().unwrap_or_default();
    let ty = first.split('$').next().unwrap_or_default();
    ty.split('|')
        .map(|t| t.trim().trim_start_matches('?'))
        .filter(|t| {
            !t.is_empty() && !["object", "mixed", "array", "string", "int", "callable"].contains(t)
        })
        .map(|t| short(t).to_owned())
        .collect()
}

fn call_args(text: &str, tokens: &[Token], open: usize) -> (Vec<String>, Vec<String>) {
    let close = matching(tokens, open);
    let mut args = Vec::new();
    let mut types = Vec::new();
    let mut depth = 0usize;
    let mut start = tokens[open].end;
    for j in open + 1..close {
        match tokens[j].kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth = depth.saturating_sub(1),
            Kind::Comma if depth == 0 => {
                args.push(text[start..tokens[j].start].trim().to_owned());
                start = tokens[j].end;
            }
            _ => {}
        }
        if (is(tokens, j, "function") || is(tokens, j, "fn"))
            && tokens.get(j + 1).is_some_and(|t| t.kind == Kind::Open('('))
            && types.is_empty()
        {
            let end = matching(tokens, j + 1);
            types = param_types(&text[tokens[j + 1].end..start_of(text, tokens, end)]);
        }
    }
    let last = text[start..start_of(text, tokens, close).max(start)].trim();
    if !last.is_empty() {
        args.push(last.to_owned());
    }
    (args, types)
}

const CALLS: [&str; 22] = [
    "listen",
    "subscribe",
    "observe",
    "macro",
    "mixin",
    "bind",
    "bindIf",
    "singleton",
    "singletonIf",
    "scoped",
    "scopedIf",
    "instance",
    "addGlobalScope",
    "composer",
    "creator",
    "share",
    "for",
    "policy",
    "define",
    "extend",
    "directive",
    "channel",
];

pub fn scan_text(path: &str, text: &str) -> Scan {
    let tokens = tokenize(text, 0, text.len());
    let lines = Lines::new(text);
    let line = |at: usize| lines.position(text, at).line + 1;
    let mut out = Scan::default();
    let mut namespace = String::new();
    let mut pending: Vec<Attribute> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if is(&tokens, i, "namespace")
            && let Some(ns) = ident(&tokens, i + 1)
        {
            namespace = ns.to_owned();
            i += 2;
            continue;
        }
        if tokens[i].kind == Kind::Hash
            && tokens.get(i + 1).is_some_and(|t| t.kind == Kind::Open('['))
        {
            let (attrs, next) = attributes(text, &tokens, i);
            pending.extend(attrs);
            i = next;
            continue;
        }
        if let Some(name) = ident(&tokens, i)
            && CALLS.contains(&name)
            && tokens.get(i + 1).is_some_and(|t| t.kind == Kind::Open('('))
            && i >= 1
            && matches!(tokens[i - 1].kind, Kind::DoubleColon | Kind::ObjectOp)
        {
            let class = match (
                &tokens[i - 1].kind,
                i.checked_sub(2).map(|k| &tokens[k].kind),
            ) {
                (Kind::DoubleColon, Some(Kind::Ident(c))) => short(c).to_owned(),
                (Kind::ObjectOp, Some(Kind::Var(v))) => format!("${v}"),
                (Kind::ObjectOp, Some(Kind::Ident(c))) => format!("->{c}"),
                (Kind::ObjectOp, Some(Kind::Close(')'))) => {
                    let mut k = i - 2;
                    let mut depth = 0usize;
                    loop {
                        match tokens[k].kind {
                            Kind::Close(_) => depth += 1,
                            Kind::Open(_) => depth = depth.saturating_sub(1),
                            _ => {}
                        }
                        if depth == 0 || k == 0 {
                            break;
                        }
                        k -= 1;
                    }
                    k.checked_sub(1)
                        .and_then(|p| ident(&tokens, p))
                        .map(|c| format!("{c}()"))
                        .unwrap_or_default()
                }
                _ => String::new(),
            };
            let (args, closure_types) = call_args(text, &tokens, i + 1);
            out.calls.push(Call {
                owner: String::new(),
                path: path.to_owned(),
                line: line(tokens[i].start),
                class,
                method: name.to_owned(),
                args,
                closure_types,
            });
            i += 2;
            continue;
        }
        let keyword = ident(&tokens, i).map(str::to_ascii_lowercase);
        let declares = matches!(
            keyword.as_deref(),
            Some("class" | "interface" | "trait" | "enum")
        ) && ident(&tokens, i + 1).is_some()
            && !i
                .checked_sub(1)
                .is_some_and(|k| tokens[k].kind == Kind::DoubleColon || is(&tokens, k, "new"))
            && !(keyword.as_deref() == Some("enum")
                && tokens.get(i + 2).is_some_and(|t| t.kind == Kind::Open('(')));
        if !declares {
            if matches!(tokens[i].kind, Kind::Semicolon | Kind::Open('{')) {
                pending.clear();
            }
            i += 1;
            continue;
        }
        let keyword = keyword.unwrap_or_default();
        let name_token = &tokens[i + 1];
        let mut symbol = Symbol {
            path: path.to_owned(),
            line: line(name_token.start),
            keyword: keyword.clone(),
            name: ident(&tokens, i + 1).unwrap_or_default().to_owned(),
            namespace: namespace.clone(),
            is_abstract: (i.saturating_sub(3)..i).any(|k| is(&tokens, k, "abstract")),
            attributes: std::mem::take(&mut pending),
            ..Symbol::default()
        };
        let mut j = i + 2;
        if keyword == "enum" && tokens.get(j).is_some_and(|t| t.kind == Kind::Colon) {
            symbol.backing = ident(&tokens, j + 1).map(str::to_owned);
            j += 2;
        }
        while j < tokens.len() && tokens[j].kind != Kind::Open('{') {
            if is(&tokens, j, "extends") {
                let (names, next) = names_until(&tokens, j + 1, |t| {
                    t.kind == Kind::Open('{')
                        || matches!(&t.kind, Kind::Ident(s) if s.eq_ignore_ascii_case("implements"))
                });
                symbol.extends = names;
                j = next;
                continue;
            }
            if is(&tokens, j, "implements") {
                let (names, next) = names_until(&tokens, j + 1, |t| t.kind == Kind::Open('{'));
                symbol.implements = names;
                j = next;
                continue;
            }
            j += 1;
        }
        if j >= tokens.len() {
            break;
        }
        let close = matching(&tokens, j);
        let mut depth = 0usize;
        let mut member_attrs: Vec<Attribute> = Vec::new();
        let mut k = j;
        while k < close {
            match &tokens[k].kind {
                Kind::Open(_) => depth += 1,
                Kind::Close(_) => depth = depth.saturating_sub(1),
                Kind::Hash
                    if depth == 1
                        && tokens.get(k + 1).is_some_and(|t| t.kind == Kind::Open('[')) =>
                {
                    let (attrs, next) = attributes(text, &tokens, k);
                    member_attrs.extend(attrs);
                    k = next;
                    continue;
                }
                Kind::Ident(w) if depth == 1 && w.eq_ignore_ascii_case("use") => {
                    let (names, next) = names_until(&tokens, k + 1, |t| {
                        matches!(t.kind, Kind::Semicolon | Kind::Open('{'))
                    });
                    symbol.traits.extend(names);
                    k = next;
                    continue;
                }
                Kind::Ident(w) if depth == 1 && w.eq_ignore_ascii_case("case") => {
                    if let Some(c) = ident(&tokens, k + 1) {
                        symbol.cases.push(c.to_owned());
                    }
                    member_attrs.clear();
                }
                Kind::Ident(w) if depth == 1 && w.eq_ignore_ascii_case("function") => {
                    if let Some(name) = ident(&tokens, k + 1)
                        && tokens.get(k + 2).is_some_and(|t| t.kind == Kind::Open('('))
                    {
                        let params_end = matching(&tokens, k + 2);
                        let mut public = true;
                        let mut is_static = false;
                        let mut b = k;
                        while b > 0 {
                            b -= 1;
                            match ident(&tokens, b).map(str::to_ascii_lowercase).as_deref() {
                                Some("private" | "protected") => public = false,
                                Some("static") => is_static = true,
                                Some("public" | "final" | "abstract") => {}
                                _ => break,
                            }
                        }
                        let mut e = params_end + 1;
                        while e < close
                            && !matches!(tokens[e].kind, Kind::Open('{') | Kind::Semicolon)
                        {
                            e += 1;
                        }
                        let body = if tokens.get(e).is_some_and(|t| t.kind == Kind::Open('{')) {
                            let end = matching(&tokens, e);
                            let b = text[tokens[e].end..start_of(text, &tokens, end)].to_owned();
                            e = end;
                            b
                        } else {
                            String::new()
                        };
                        symbol.methods.push(Method {
                            name: name.to_owned(),
                            line: line(tokens[k + 1].start),
                            public,
                            is_static,
                            params: text[tokens[k + 2].end..start_of(text, &tokens, params_end)]
                                .trim()
                                .to_owned(),
                            body,
                            attributes: std::mem::take(&mut member_attrs),
                        });
                        k = e + 1;
                        continue;
                    }
                }
                Kind::Semicolon if depth == 1 => member_attrs.clear(),
                _ => {}
            }
            k += 1;
        }
        let body_scan = scan_calls(path, &symbol.name, text, &tokens[j + 1..close], &lines);
        out.calls.extend(body_scan);
        out.symbols.push(symbol);
        i = close + 1;
    }
    out
}

fn scan_calls(path: &str, owner: &str, text: &str, tokens: &[Token], lines: &Lines) -> Vec<Call> {
    let mut out = Vec::new();
    for i in 1..tokens.len() {
        let Some(name) = ident(tokens, i) else {
            continue;
        };
        if !CALLS.contains(&name)
            || !tokens.get(i + 1).is_some_and(|t| t.kind == Kind::Open('('))
            || !matches!(tokens[i - 1].kind, Kind::DoubleColon | Kind::ObjectOp)
        {
            continue;
        }
        let class = match (
            &tokens[i - 1].kind,
            i.checked_sub(2).map(|k| &tokens[k].kind),
        ) {
            (Kind::DoubleColon, Some(Kind::Ident(c))) => short(c).to_owned(),
            (Kind::ObjectOp, Some(Kind::Var(v))) => format!("${v}"),
            (Kind::ObjectOp, Some(Kind::Ident(c))) => format!("->{c}"),
            (Kind::ObjectOp, Some(Kind::Close(')'))) => i
                .checked_sub(4)
                .and_then(|k| ident(tokens, k))
                .map(|c| format!("{c}()"))
                .unwrap_or_default(),
            _ => String::new(),
        };
        let (args, closure_types) = call_args(text, tokens, i + 1);
        out.push(Call {
            owner: owner.to_owned(),
            path: path.to_owned(),
            line: lines.position(text, tokens[i].start).line + 1,
            class,
            method: name.to_owned(),
            args,
            closure_types,
        });
    }
    out
}

pub fn role(s: &Symbol) -> &'static str {
    let path = s.path.as_str();
    let extends = |names: &[&str]| s.extends.iter().any(|e| names.contains(&e.as_str()));
    let implements = |names: &[&str]| s.implements.iter().any(|e| names.contains(&e.as_str()));
    match s.keyword.as_str() {
        "trait" => return "Trait",
        "interface" => return "Interface",
        "enum" => return "Enum",
        _ => {}
    }
    if extends(&["Model", "Authenticatable", "Pivot", "MorphPivot", "User"])
        && !path.contains("/Http/")
    {
        "Model"
    } else if path.contains("/Http/Controllers/") || extends(&["Controller"]) {
        "Controller"
    } else if path.contains("/Http/Middleware/") {
        "Middleware"
    } else if extends(&["FormRequest"]) || path.contains("/Http/Requests/") {
        "Form request"
    } else if extends(&["JsonResource", "ResourceCollection"]) || path.contains("/Http/Resources/")
    {
        "API resource"
    } else if extends(&[
        "ServiceProvider",
        "EventServiceProvider",
        "AuthServiceProvider",
        "RouteServiceProvider",
    ]) {
        "Provider"
    } else if extends(&["Command"]) || path.contains("/Console/Commands/") {
        "Command"
    } else if extends(&["Mailable"]) {
        "Mailable"
    } else if extends(&["Notification"]) {
        "Notification"
    } else if path.contains("/Listeners/") {
        "Listener"
    } else if path.contains("/Events/") {
        "Event"
    } else if path.contains("/Observers/") || s.name.ends_with("Observer") {
        "Observer"
    } else if path.contains("/Policies/") || s.name.ends_with("Policy") {
        "Policy"
    } else if implements(&["ShouldQueue"]) || path.contains("/Jobs/") {
        "Job"
    } else if implements(&[
        "ValidationRule",
        "Rule",
        "InvokableRule",
        "DataAwareRule",
        "ValidatorAwareRule",
    ]) || path.contains("/Rules/")
    {
        "Rule"
    } else if implements(&[
        "CastsAttributes",
        "CastsInboundAttributes",
        "Castable",
        "SerializesCastableAttributes",
    ]) || path.contains("/Casts/")
    {
        "Cast"
    } else if implements(&["Scope"]) || path.contains("/Scopes/") {
        "Scope"
    } else if extends(&["Factory"]) || path.starts_with("database/factories/") {
        "Factory"
    } else if extends(&["Seeder"]) || path.starts_with("database/seeders/") {
        "Seeder"
    } else if path.contains("/View/Components/")
        || extends(&["Component"]) && !path.contains("/Livewire/")
    {
        "Blade component"
    } else if path.contains("/Livewire/") {
        "Livewire"
    } else if s
        .extends
        .iter()
        .any(|e| e.ends_with("Exception") || e == "Error")
        || path.contains("/Exceptions/")
    {
        "Exception"
    } else if path.contains("/Actions/") {
        "Action"
    } else if path.contains("/Services/") {
        "Service"
    } else if path.contains("/Data/") || path.contains("/DTO") || extends(&["Data"]) {
        "Data object"
    } else if path.contains("/Broadcasting/") {
        "Channel"
    } else {
        "Class"
    }
}

pub fn folder(path: &str) -> String {
    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    dir.to_owned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listener {
    pub listener: String,
    pub method: String,
    pub events: Vec<String>,
    pub queued: bool,
    pub via: String,
    pub path: String,
    pub line: u32,
}

fn listener_arg(arg: &str) -> Option<(String, String)> {
    if arg.starts_with('[') {
        let refs = class_refs(arg);
        let method = arg.split(['\'', '"']).nth(1).unwrap_or("handle").to_owned();
        return refs.first().map(|c| (c.clone(), method));
    }
    if arg.starts_with("function") || arg.starts_with("fn") || arg.starts_with("static") {
        return Some(("closure".into(), String::new()));
    }
    class_refs(arg)
        .first()
        .map(|c| (c.clone(), "handle".into()))
        .or_else(|| {
            let s = arg.trim_matches(['\'', '"']);
            (!s.is_empty()).then(|| {
                let (c, m) = s.split_once('@').unwrap_or((s, "handle"));
                (short(c).to_owned(), m.to_owned())
            })
        })
}

pub fn listeners(scan: &Scan) -> Vec<Listener> {
    let mut out: Vec<Listener> = Vec::new();
    let by_name: BTreeMap<&str, &Symbol> =
        scan.symbols.iter().map(|s| (s.name.as_str(), s)).collect();
    let queued = |name: &str| {
        by_name.get(name).is_some_and(|s| {
            s.implements
                .iter()
                .any(|i| i == "ShouldQueue" || i == "ShouldQueueAfterCommit")
        })
    };
    for s in scan.symbols.iter().filter(|s| {
        role(s) == "Listener"
            || s.methods
                .iter()
                .any(|m| m.attributes.iter().any(|a| a.name == "ListensTo"))
    }) {
        for m in &s.methods {
            let attributed: Vec<String> = m
                .attributes
                .iter()
                .filter(|a| a.name == "ListensTo")
                .flat_map(|a| class_refs(&a.args))
                .collect();
            let handles = m.name == "handle"
                || m.name == "__invoke"
                || (m.name.starts_with("handle") && role(s) == "Listener" && m.public);
            if !handles && attributed.is_empty() {
                continue;
            }
            let mut events = attributed;
            if events.is_empty() {
                events = param_types(&m.params);
            }
            if events.is_empty() {
                continue;
            }
            out.push(Listener {
                listener: s.name.clone(),
                method: m.name.clone(),
                events,
                queued: s
                    .implements
                    .iter()
                    .any(|i| i == "ShouldQueue" || i == "ShouldQueueAfterCommit"),
                via: "discovered".into(),
                path: s.path.clone(),
                line: m.line,
            });
        }
    }
    for c in scan.calls.iter().filter(|c| {
        c.method == "listen"
            && (c.class == "Event"
                || c.class.starts_with('$')
                || c.class == "->events"
                || c.class == "events()")
    }) {
        let Some(first) = c.args.first() else {
            continue;
        };
        let closure =
            first.starts_with("function") || first.starts_with("fn") || first.starts_with("static");
        let events = if closure {
            c.closure_types.clone()
        } else {
            class_refs(first)
                .into_iter()
                .chain(
                    (!first.contains("::class"))
                        .then(|| first.trim_matches(['\'', '"']).to_owned())
                        .filter(|s| !s.is_empty() && !s.starts_with('[')),
                )
                .collect()
        };
        let (listener, method) = if closure {
            ("closure".to_owned(), String::new())
        } else {
            c.args
                .get(1)
                .and_then(|a| listener_arg(a))
                .unwrap_or_else(|| ("closure".into(), String::new()))
        };
        out.push(Listener {
            queued: queued(&listener),
            listener,
            method,
            events,
            via: "Event::listen".into(),
            path: c.path.clone(),
            line: c.line,
        });
    }
    for s in &scan.symbols {
        if let Some(subscribe) = s.methods.iter().find(|m| m.name == "subscribe") {
            let part = scan_text(&s.path, &format!("<?php {}", subscribe.body));
            for c in part.calls.iter().filter(|c| c.method == "listen") {
                let events = c.args.first().map(|a| class_refs(a)).unwrap_or_default();
                let (_, method) = c
                    .args
                    .get(1)
                    .and_then(|a| listener_arg(a))
                    .unwrap_or_else(|| (s.name.clone(), "handle".into()));
                out.push(Listener {
                    listener: s.name.clone(),
                    method,
                    events,
                    queued: queued(&s.name),
                    via: "subscriber".into(),
                    path: s.path.clone(),
                    line: subscribe.line,
                });
            }
            for (key, value) in array_pairs(&subscribe.body) {
                let events = class_refs(&key);
                if events.is_empty() {
                    continue;
                }
                out.push(Listener {
                    listener: s.name.clone(),
                    method: value.split(['\'', '"']).nth(1).unwrap_or(&value).to_owned(),
                    events,
                    queued: queued(&s.name),
                    via: "subscriber".into(),
                    path: s.path.clone(),
                    line: subscribe.line,
                });
            }
        }
    }
    out
}

pub fn array_pairs(body: &str) -> Vec<(String, String)> {
    let tokens = tokenize(body, 0, body.len());
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != Kind::DoubleArrow || i < 3 {
            continue;
        }
        let key_start = (0..i)
            .rev()
            .find(|&k| matches!(tokens[k].kind, Kind::Comma | Kind::Open('[')))
            .map_or(0, |k| k + 1);
        let value_end = (i + 1..tokens.len())
            .find(|&k| matches!(tokens[k].kind, Kind::Comma | Kind::Close(']')))
            .unwrap_or(tokens.len() - 1);
        if key_start >= i || value_end <= i + 1 {
            continue;
        }
        let key = body[tokens[key_start].start..tokens[i - 1].end].to_owned();
        let value = body[tokens[i + 1].start..tokens[value_end - 1].end].to_owned();
        out.push((key, value));
    }
    out
}

pub fn listen_property(project: &Project, root: &str) -> Vec<Listener> {
    let mut out = Vec::new();
    for file in find(project, root, "app/Providers/**/*.php", 200) {
        let Ok(text) = crate::project::read_listed(project, &file) else {
            continue;
        };
        let Some(at) = text.find("$listen") else {
            continue;
        };
        let Some(open) = text[at..].find('[').map(|o| at + o) else {
            continue;
        };
        let tokens = tokenize(&text, open, text.len());
        if tokens.is_empty() {
            continue;
        }
        let close = matching(&tokens, 0);
        let body = &text[open..tokens.get(close).map_or(text.len(), |t| t.end)];
        let lines = Lines::new(&text);
        let line = lines.position(&text, at).line + 1;
        let inner = tokenize(body, 0, body.len());
        let mut depth = 0usize;
        let mut event: Option<String> = None;
        for (i, t) in inner.iter().enumerate() {
            match t.kind {
                Kind::Open('[') => depth += 1,
                Kind::Close(']') => depth = depth.saturating_sub(1),
                Kind::DoubleArrow if depth == 1 => {
                    let key = &body[inner[i.saturating_sub(3)].start..inner[i - 1].end];
                    event = class_refs(key).pop().or_else(|| match &inner[i - 1].kind {
                        Kind::Str { value, .. } => Some(value.clone()),
                        _ => None,
                    });
                }
                Kind::DoubleColon if depth == 2 && is(&inner, i + 1, "class") => {
                    if let (Some(ev), Some(l)) = (event.clone(), ident(&inner, i - 1)) {
                        out.push(Listener {
                            listener: short(l).to_owned(),
                            method: "handle".into(),
                            events: vec![ev],
                            queued: false,
                            via: "$listen".into(),
                            path: rel(root, &file).to_owned(),
                            line,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observer {
    pub observer: String,
    pub models: Vec<String>,
    pub hooks: Vec<String>,
    pub via: String,
    pub path: String,
    pub line: u32,
}

const MODEL_EVENTS: [&str; 16] = [
    "retrieved",
    "creating",
    "created",
    "updating",
    "updated",
    "saving",
    "saved",
    "deleting",
    "deleted",
    "trashed",
    "forceDeleting",
    "forceDeleted",
    "restoring",
    "restored",
    "replicating",
    "booted",
];

pub fn observers(scan: &Scan) -> Vec<Observer> {
    let mut models: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    for s in &scan.symbols {
        for a in s.attributes.iter().filter(|a| a.name == "ObservedBy") {
            for o in class_refs(&a.args) {
                let e = models.entry(o).or_default();
                e.0.insert(s.name.clone());
                e.1.insert("#[ObservedBy]".into());
            }
        }
    }
    for c in scan.calls.iter().filter(|c| c.method == "observe") {
        for o in c.args.iter().flat_map(|a| class_refs(a)) {
            let e = models.entry(o).or_default();
            if !c.class.is_empty() && !c.class.starts_with('$') && !c.class.starts_with("->") {
                e.0.insert(c.class.clone());
            }
            e.1.insert("::observe()".into());
        }
    }
    let mut out = Vec::new();
    let candidates: Vec<&Symbol> = scan
        .symbols
        .iter()
        .filter(|s| s.keyword == "class" && (role(s) == "Observer" || models.contains_key(&s.name)))
        .collect();
    for s in candidates {
        let (m, via) = models.remove(&s.name).unwrap_or_default();
        let guessed = s
            .name
            .strip_suffix("Observer")
            .filter(|_| m.is_empty())
            .map(str::to_owned);
        out.push(Observer {
            observer: s.name.clone(),
            models: if m.is_empty() {
                guessed.into_iter().collect()
            } else {
                m.into_iter().collect()
            },
            hooks: s
                .methods
                .iter()
                .filter(|x| x.public && MODEL_EVENTS.contains(&x.name.as_str()))
                .map(|x| x.name.clone())
                .collect(),
            via: if via.is_empty() {
                "not registered".into()
            } else {
                via.into_iter().collect::<Vec<_>>().join(", ")
            },
            path: s.path.clone(),
            line: s.line,
        });
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub abstract_: String,
    pub concrete: String,
    pub lifetime: String,
    pub path: String,
    pub line: u32,
}

pub fn bindings(scan: &Scan) -> Vec<Binding> {
    let mut out = Vec::new();
    for c in &scan.calls {
        let lifetime = match c.method.as_str() {
            "bind" | "bindIf" => "transient",
            "singleton" | "singletonIf" => "singleton",
            "scoped" | "scopedIf" => "scoped",
            "instance" => "instance",
            _ => continue,
        };
        let app = c.class == "App"
            || c.class == "->app"
            || c.class == "app()"
            || c.class == "$app"
            || c.class == "Container"
            || c.class == "$container";
        if !app {
            continue;
        }
        let abstract_ = c
            .args
            .first()
            .map(|a| {
                class_refs(a)
                    .pop()
                    .unwrap_or_else(|| a.trim_matches(['\'', '"']).to_owned())
            })
            .unwrap_or_default();
        let concrete = match c.args.get(1) {
            None => "itself".to_owned(),
            Some(a)
                if a.starts_with("function") || a.starts_with("fn") || a.starts_with("static") =>
            {
                let made = class_refs(a);
                if made.is_empty() {
                    "closure".into()
                } else {
                    format!("closure → {}", made.join(", "))
                }
            }
            Some(a) => class_refs(a).pop().unwrap_or_else(|| a.clone()),
        };
        out.push(Binding {
            abstract_,
            concrete,
            lifetime: format!(
                "{lifetime}{}",
                if c.method.ends_with("If") {
                    " (if)"
                } else {
                    ""
                }
            ),
            path: c.path.clone(),
            line: c.line,
        });
    }
    for s in &scan.symbols {
        for a in &s.attributes {
            let lifetime = match a.name.as_str() {
                "Bind" => "transient",
                "Singleton" => "singleton",
                "Scoped" => "scoped",
                _ => continue,
            };
            let concrete = if a.name == "Bind" {
                class_refs(&a.args).first().cloned().unwrap_or_default()
            } else {
                s.name.clone()
            };
            out.push(Binding {
                abstract_: s.name.clone(),
                concrete,
                lifetime: format!("{lifetime} (#[{}])", a.name),
                path: s.path.clone(),
                line: s.line,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_symbols_and_calls() {
        let text = r#"<?php
namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\ObservedBy;

#[ObservedBy([UserObserver::class])]
#[ScopedBy(ActiveScope::class)]
abstract class User extends Authenticatable implements MustVerifyEmail, HasMedia
{
    use HasFactory, Notifiable;
    use SoftDeletes { restore as protected; }

    #[Scope]
    protected function verified(Builder $q): void {}

    public function handle(OrderShipped|OrderCancelled $event) { Event::listen(fn (Paid $e) => 1); }
}

enum Status: string
{
    case Active = 'active';
    case Banned = 'banned';
}

interface Repo extends Countable, ArrayAccess {}
"#;
        let scan = scan_text("app/Models/User.php", text);
        let user = &scan.symbols[0];
        assert_eq!(user.name, "User");
        assert_eq!(user.namespace, "App\\Models");
        assert!(user.is_abstract);
        assert_eq!(user.extends, ["Authenticatable"]);
        assert_eq!(user.implements, ["MustVerifyEmail", "HasMedia"]);
        assert_eq!(user.traits, ["HasFactory", "Notifiable", "SoftDeletes"]);
        assert_eq!(
            user.attributes
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            ["ObservedBy", "ScopedBy"]
        );
        assert_eq!(user.methods[0].attributes[0].name, "Scope");
        assert_eq!(
            param_types(&user.methods[1].params),
            ["OrderShipped", "OrderCancelled"]
        );
        assert_eq!(role(user), "Model");
        let status = &scan.symbols[1];
        assert_eq!(
            (status.keyword.as_str(), status.backing.as_deref()),
            ("enum", Some("string"))
        );
        assert_eq!(status.cases, ["Active", "Banned"]);
        assert_eq!(scan.symbols[2].extends, ["Countable", "ArrayAccess"]);
        let listen = scan.calls.iter().find(|c| c.method == "listen").unwrap();
        assert_eq!(listen.closure_types, ["Paid"]);
        let observers = observers(&scan);
        assert_eq!(observers.len(), 0);
    }

    #[test]
    fn finds_listeners_observers_and_bindings() {
        let mut scan = scan_text(
            "app/Listeners/SendShipmentNotification.php",
            "<?php\nclass SendShipmentNotification implements ShouldQueue {\n public function handle(OrderShipped $event): void {}\n}\n",
        );
        let provider = scan_text(
            "app/Providers/AppServiceProvider.php",
            "<?php\nclass AppServiceProvider extends ServiceProvider {\n public function register(): void {\n  $this->app->singleton(Payments::class, fn () => new StripePayments);\n  $this->app->bind(Clock::class, SystemClock::class);\n }\n public function boot(): void {\n  Event::listen(PodcastProcessed::class, [SendPodcastNotification::class, 'handle']);\n  Event::listen(function (UserDeleted $e) {});\n  Order::observe(OrderObserver::class);\n }\n}\n",
        );
        let observer = scan_text(
            "app/Observers/OrderObserver.php",
            "<?php\nclass OrderObserver {\n public function created(Order $o): void {}\n public function deleted(Order $o): void {}\n private function helper() {}\n}\n",
        );
        for part in [provider, observer] {
            scan.symbols.extend(part.symbols);
            scan.calls.extend(part.calls);
        }
        let l = listeners(&scan);
        let summary: Vec<(String, Vec<String>, bool, String)> = l
            .iter()
            .map(|x| {
                (
                    x.listener.clone(),
                    x.events.clone(),
                    x.queued,
                    x.via.clone(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "SendShipmentNotification".to_owned(),
                    vec!["OrderShipped".to_owned()],
                    true,
                    "discovered".to_owned()
                ),
                (
                    "SendPodcastNotification".to_owned(),
                    vec!["PodcastProcessed".to_owned()],
                    false,
                    "Event::listen".to_owned()
                ),
                (
                    "closure".to_owned(),
                    vec!["UserDeleted".to_owned()],
                    false,
                    "Event::listen".to_owned()
                ),
            ]
        );
        let o = observers(&scan);
        assert_eq!(o[0].models, ["Order"]);
        assert_eq!(o[0].hooks, ["created", "deleted"]);
        assert_eq!(o[0].via, "::observe()");
        let b = bindings(&scan);
        assert_eq!(
            b.iter()
                .map(|x| (
                    x.abstract_.as_str(),
                    x.concrete.as_str(),
                    x.lifetime.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                ("Payments", "closure → StripePayments", "singleton"),
                ("Clock", "SystemClock", "transient")
            ]
        );
    }
}
