use crate::querysql::{Kind, Tok, tokenize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub title: String,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub start: usize,
    pub end: usize,
    pub message: String,
    pub code: &'static str,
    pub fixes: Vec<Fix>,
    pub info: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ModelFacts {
    pub name: String,
    pub table: String,
    pub path: String,
    pub source: String,
    pub relations: Vec<String>,
}

pub type Columns = BTreeMap<String, BTreeSet<String>>;
pub type Parts = (Vec<ModelFacts>, Columns, Vec<(String, String)>);

#[derive(Clone, Copy, Debug)]
pub struct Enabled {
    pub columns: bool,
    pub env: bool,
    pub fillable: bool,
    pub n_plus_one: bool,
    pub duplicate_keys: bool,
}

pub struct Facts<'a> {
    pub path: &'a str,
    pub rel_path: &'a str,
    pub models: &'a [ModelFacts],
    pub columns: &'a BTreeMap<String, BTreeSet<String>>,
    pub config_env: &'a [(String, String)],
    pub enabled: Enabled,
}

impl Facts<'_> {
    fn model(&self, class: &str) -> Option<&ModelFacts> {
        let short = class.rsplit('\\').next().unwrap_or(class);
        self.models.iter().find(|m| m.name == short)
    }
}

fn is_sym(t: Option<&Tok>, s: &str) -> bool {
    t.is_some_and(|t| t.kind == Kind::Sym && t.text == s)
}

fn closing(toks: &[Tok], open: usize) -> usize {
    let mut depth = 0i32;
    for (i, t) in toks.iter().enumerate().skip(open) {
        if t.kind != Kind::Sym {
            continue;
        }
        match t.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    toks.len().saturating_sub(1)
}

#[derive(Clone, Debug)]
struct Call {
    name: String,
    name_at: usize,
    open: usize,
    close: usize,
}

#[derive(Clone, Debug)]
struct Chain {
    class: String,

    calls: Vec<Call>,
    end: usize,
}

fn chain_at(toks: &[Tok], i: usize) -> Option<Chain> {
    let head = toks.get(i)?;
    if head.kind != Kind::Ident
        || !is_sym(toks.get(i + 1), "::")
        || toks
            .get(i + 2)
            .is_none_or(|t| t.kind != Kind::Ident || t.text == "class")
        || !is_sym(toks.get(i + 3), "(")
    {
        return None;
    }
    if i > 0 && (is_sym(toks.get(i - 1), "->") || is_sym(toks.get(i - 1), "::")) {
        return None;
    }
    let mut calls = Vec::new();
    let close = closing(toks, i + 3);
    calls.push(Call {
        name: toks[i + 2].text.clone(),
        name_at: i + 2,
        open: i + 3,
        close,
    });
    let mut at = close + 1;
    while (is_sym(toks.get(at), "->") || is_sym(toks.get(at), "?->"))
        && toks.get(at + 1).is_some_and(|t| t.kind == Kind::Ident)
        && is_sym(toks.get(at + 2), "(")
    {
        let close = closing(toks, at + 2);
        calls.push(Call {
            name: toks[at + 1].text.clone(),
            name_at: at + 1,
            open: at + 2,
            close,
        });
        at = close + 1;
    }
    Some(Chain {
        class: head.text.clone(),
        calls,
        end: at,
    })
}

fn chains(toks: &[Tok]) -> Vec<Chain> {
    (0..toks.len()).filter_map(|i| chain_at(toks, i)).collect()
}

fn first_string(toks: &[Tok], call: &Call) -> Option<usize> {
    let i = call.open + 1;
    (toks.get(i)?.kind == Kind::Str
        && (is_sym(toks.get(i + 1), ",") || is_sym(toks.get(i + 1), ")")))
    .then_some(i)
}

pub(crate) fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j] + usize::from(ca != *cb))
                    .min(prev[j + 1] + 1)
                    .min(cur[j] + 1),
            );
        }
        prev = cur;
    }
    prev[b.len()]
}

fn closest<'a>(name: &str, options: impl Iterator<Item = &'a String>) -> Option<&'a String> {
    let limit = (name.len() / 3).max(2);
    options
        .map(|o| (distance(name, o), o))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, o)| (*d, o.len()))
        .map(|(_, o)| o)
}

const COLUMN_METHODS: &[&str] = &[
    "where",
    "orWhere",
    "whereNot",
    "orWhereNot",
    "whereIn",
    "orWhereIn",
    "whereNotIn",
    "orWhereNotIn",
    "whereNull",
    "orWhereNull",
    "whereNotNull",
    "orWhereNotNull",
    "whereBetween",
    "orWhereBetween",
    "whereNotBetween",
    "whereDate",
    "whereMonth",
    "whereDay",
    "whereYear",
    "whereTime",
    "whereLike",
    "orWhereLike",
    "whereNotLike",
    "orderBy",
    "orderByDesc",
    "latest",
    "oldest",
    "pluck",
    "value",
    "increment",
    "decrement",
    "firstWhere",
    "sum",
    "avg",
    "min",
    "max",
    "whereColumn",
    "groupBy",
];

const ALIASING: &[&str] = &[
    "select",
    "addSelect",
    "selectRaw",
    "selectSub",
    "join",
    "leftJoin",
    "rightJoin",
    "crossJoin",
    "joinSub",
    "leftJoinSub",
    "from",
    "fromSub",
    "withCount",
    "withSum",
    "withAvg",
    "withMin",
    "withMax",
    "withExists",
    "withAggregate",
    "having",
    "havingRaw",
    "union",
    "unionAll",
    "table",
    "fromRaw",
];

fn unknown_columns(toks: &[Tok], facts: &Facts, out: &mut Vec<Finding>) {
    for chain in chains(toks) {
        let table = if chain.class.trim_start_matches('\\') == "DB" || chain.class.ends_with("\\DB")
        {
            let first = &chain.calls[0];
            if first.name != "table" {
                continue;
            }
            match first_string(toks, first) {
                Some(i) if !toks[i].text.contains(' ') => toks[i].text.clone(),
                _ => continue,
            }
        } else {
            match facts.model(&chain.class) {
                Some(m) => m.table.clone(),
                None => continue,
            }
        };
        let Some(columns) = facts.columns.get(&table).filter(|c| !c.is_empty()) else {
            continue;
        };
        let db = chain.class.trim_start_matches('\\') == "DB" || chain.class.ends_with("\\DB");
        if chain
            .calls
            .iter()
            .skip(usize::from(db))
            .any(|c| ALIASING.contains(&c.name.as_str()))
        {
            continue;
        }
        for call in &chain.calls {
            if !COLUMN_METHODS.contains(&call.name.as_str()) {
                continue;
            }
            let Some(i) = first_string(toks, call) else {
                continue;
            };
            let name = &toks[i].text;
            if name.is_empty()
                || name == "*"
                || name.contains(['.', ' ', '(', '-', '>'])
                || columns.contains(name)
            {
                continue;
            }
            let (start, end) = (toks[i].start + 1, toks[i].end.saturating_sub(1));
            let mut message = format!("Column '{name}' doesn't exist in table '{table}'");
            let mut fixes = Vec::new();
            if let Some(best) = closest(name, columns.iter()) {
                message.push_str(&format!(": did you mean '{best}'?"));
                fixes.push(Fix {
                    title: format!("Change to '{best}'"),
                    edits: vec![Edit {
                        path: facts.path.to_owned(),
                        start,
                        end,
                        text: best.clone(),
                    }],
                });
            }
            out.push(Finding {
                start,
                end,
                message,
                code: "laravel.unknown-column",
                fixes,
                info: false,
            });
        }
    }
}

fn env_calls(toks: &[Tok], facts: &Facts, out: &mut Vec<Finding>) {
    let rel = facts.rel_path.replace('\\', "/");
    if rel.starts_with("config/") || rel.contains("vendor/") || rel.starts_with("bootstrap/cache/")
    {
        return;
    }
    for i in 0..toks.len() {
        let t = &toks[i];
        if t.kind != Kind::Ident
            || t.text.trim_start_matches('\\') != "env"
            || !is_sym(toks.get(i + 1), "(")
        {
            continue;
        }
        if i > 0
            && toks[i - 1].kind == Kind::Sym
            && matches!(toks[i - 1].text.as_str(), "->" | "?->" | "::")
        {
            continue;
        }
        if i > 0
            && toks[i - 1].kind == Kind::Ident
            && matches!(toks[i - 1].text.as_str(), "function" | "fn" | "new")
        {
            continue;
        }
        let Some(key) = toks
            .get(i + 2)
            .filter(|t| t.kind == Kind::Str)
            .map(|t| t.text.clone())
        else {
            continue;
        };
        let close = closing(toks, i + 1);
        let (start, end) = (t.start, toks[close].end);
        let keys: Vec<&String> = facts
            .config_env
            .iter()
            .filter(|(_, v)| *v == key)
            .map(|(k, _)| k)
            .take(3)
            .collect();
        let fixes = keys
            .iter()
            .map(|k| Fix {
                title: format!("Replace with config('{k}')"),
                edits: vec![Edit {
                    path: facts.path.to_owned(),
                    start,
                    end,
                    text: format!("config('{k}')"),
                }],
            })
            .collect();
        let hint = match keys.first() {
            Some(k) => format!(": use config('{k}')"),
            None => ": read it in a config file and use config() here".to_owned(),
        };
        out.push(Finding {
            start,
            end,
            message: format!("env('{key}') returns null once the configuration is cached (php artisan config:cache){hint}"),
            code: "laravel.env-outside-config",
            fixes,
            info: false,
        });
    }
}

fn string_list(source: &str, from: usize) -> Option<(Vec<String>, usize, usize)> {
    let open = from + source[from..].find('[')?;
    let toks = tokenize(&source[open..]);
    let close = closing(&toks, 0);
    let names = toks[1..close]
        .iter()
        .filter(|t| t.kind == Kind::Str)
        .map(|t| t.text.clone())
        .collect();
    Some((names, open, open + toks[close].start))
}

fn fillable_of(source: &str) -> Option<(Vec<String>, usize, usize)> {
    if let Some(i) = source.find("$fillable") {
        return string_list(source, i);
    }
    source
        .find("#[Fillable(")
        .and_then(|i| string_list(source, i))
}

fn guarded_of(source: &str) -> Option<Vec<String>> {
    let i = source
        .find("$guarded")
        .or_else(|| source.find("#[Guarded("))?;
    string_list(source, i).map(|(names, _, _)| names)
}

fn insert_into_list(source: &str, open: usize, close: usize, item: &str) -> (usize, String) {
    let inner = &source[open + 1..close];
    let trimmed = inner.trim_end();
    let at = open + 1 + trimmed.len();
    if inner.contains('\n') {
        let indent: String = trimmed
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
            .unwrap_or_else(|| "    ".to_owned());
        let comma = if trimmed.trim().is_empty() || trimmed.ends_with(',') {
            ""
        } else {
            ","
        };
        (at, format!("{comma}\n{indent}'{item}',"))
    } else if trimmed.trim().is_empty() {
        (at, format!("'{item}'"))
    } else if trimmed.ends_with(',') {
        (at, format!(" '{item}'"))
    } else {
        (at, format!(", '{item}'"))
    }
}

fn array_keys(toks: &[Tok], open: usize) -> Vec<usize> {
    let close = closing(toks, open);
    let mut depth = 0;
    let mut out = Vec::new();
    for i in open + 1..close {
        let t = &toks[i];
        if t.kind == Kind::Sym {
            match t.text.as_str() {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                _ => {}
            }
            continue;
        }
        if depth == 0 && t.kind == Kind::Str && is_sym(toks.get(i + 1), "=>") {
            out.push(i);
        }
    }
    out
}

const FILLING: &[&str] = &[
    "create",
    "make",
    "firstOrCreate",
    "firstOrNew",
    "updateOrCreate",
    "createQuietly",
];

fn mass_assignment(toks: &[Tok], facts: &Facts, out: &mut Vec<Finding>) {
    for chain in chains(toks) {
        let Some(model) = facts.model(&chain.class) else {
            continue;
        };
        let fillable = fillable_of(&model.source);
        let guarded =
            guarded_of(&model.source).filter(|g| !g.is_empty() && !g.iter().any(|x| x == "*"));
        if fillable.is_none() && guarded.is_none() {
            continue;
        }
        for call in &chain.calls {
            if !FILLING.contains(&call.name.as_str()) {
                continue;
            }
            let mut arrays = Vec::new();
            let mut i = call.open + 1;
            while i < call.close {
                if is_sym(toks.get(i), "[") {
                    arrays.push(i);
                    i = closing(toks, i) + 1;
                } else {
                    i += 1;
                }
            }
            for open in arrays {
                for k in array_keys(toks, open) {
                    let key = &toks[k].text;
                    let (start, end) = (toks[k].start + 1, toks[k].end.saturating_sub(1));
                    if let Some((names, list_open, list_close)) = &fillable {
                        if names.contains(key) {
                            continue;
                        }
                        let (at, text) =
                            insert_into_list(&model.source, *list_open, *list_close, key);
                        out.push(Finding {
                            start,
                            end,
                            message: format!(
                                "'{key}' isn't in the $fillable of {}: {}() leaves it out",
                                model.name, call.name
                            ),
                            code: "laravel.not-fillable",
                            fixes: vec![Fix {
                                title: format!("Add '{key}' to $fillable of {}", model.name),
                                edits: vec![Edit {
                                    path: model.path.clone(),
                                    start: at,
                                    end: at,
                                    text,
                                }],
                            }],
                            info: false,
                        });
                    } else if let Some(g) = &guarded
                        && g.contains(key)
                    {
                        out.push(Finding {
                            start,
                            end,
                            message: format!(
                                "'{key}' is in the $guarded of {}: {}() leaves it out",
                                model.name, call.name
                            ),
                            code: "laravel.not-fillable",
                            fixes: Vec::new(),
                            info: false,
                        });
                    }
                }
            }
        }
    }
}

const COLLECTING: &[&str] = &[
    "get",
    "all",
    "paginate",
    "simplePaginate",
    "cursorPaginate",
    "cursor",
    "lazy",
    "findMany",
    "lazyById",
];

fn eager(toks: &[Tok], chain: &Chain) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for call in &chain.calls {
        if !matches!(
            call.name.as_str(),
            "with" | "load" | "withWhereHas" | "loadMissing"
        ) {
            continue;
        }
        for t in &toks[call.open + 1..call.close] {
            if t.kind == Kind::Str {
                out.insert(
                    t.text
                        .split(['.', ':'])
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
        }
    }
    out
}

fn n_plus_one(toks: &[Tok], facts: &Facts, out: &mut Vec<Finding>) {
    for i in 0..toks.len() {
        if toks[i].kind != Kind::Ident || toks[i].text != "foreach" || !is_sym(toks.get(i + 1), "(")
        {
            continue;
        }
        let head_close = closing(toks, i + 1);
        let Some(as_at) = (i + 2..head_close)
            .find(|&j| toks[j].kind == Kind::Ident && toks[j].text.eq_ignore_ascii_case("as"))
        else {
            continue;
        };
        let Some(item) = toks[as_at + 1..head_close]
            .iter()
            .rev()
            .find(|t| t.kind == Kind::Var)
            .map(|t| t.text.clone())
        else {
            continue;
        };
        let source = if as_at == i + 3 && toks[i + 2].kind == Kind::Var {
            let var = &toks[i + 2].text;
            (0..i)
                .rev()
                .find(|&j| {
                    toks[j].kind == Kind::Var
                        && toks[j].text == *var
                        && is_sym(toks.get(j + 1), "=")
                })
                .and_then(|j| chain_at(toks, j + 2))
        } else {
            chain_at(toks, i + 2).filter(|c| c.end == as_at)
        };
        let Some(chain) = source else { continue };
        let Some(model) = facts.model(&chain.class) else {
            continue;
        };
        let Some(terminal) = chain
            .calls
            .iter()
            .rposition(|c| COLLECTING.contains(&c.name.as_str()))
        else {
            continue;
        };
        if terminal + 1 != chain.calls.len() {
            continue;
        }
        let loaded = eager(toks, &chain);
        if !is_sym(toks.get(head_close + 1), "{") {
            continue;
        }
        let body_close = closing(toks, head_close + 1);
        let mut seen = BTreeSet::new();
        for j in head_close + 2..body_close {
            if toks[j].kind != Kind::Var
                || toks[j].text != item
                || !(is_sym(toks.get(j + 1), "->") || is_sym(toks.get(j + 1), "?->"))
            {
                continue;
            }
            let Some(name) = toks.get(j + 2).filter(|t| t.kind == Kind::Ident) else {
                continue;
            };
            if is_sym(toks.get(j + 3), "(")
                || !model.relations.contains(&name.text)
                || loaded.contains(&name.text)
            {
                continue;
            }
            if !seen.insert(name.text.clone()) {
                continue;
            }
            let rel = &name.text;
            let term = &chain.calls[terminal];
            let edits = if term.name == "all" && terminal == 0 {
                vec![Edit {
                    path: facts.path.to_owned(),
                    start: toks[term.name_at].start,
                    end: toks[term.name_at].end,
                    text: format!("with('{rel}')->get"),
                }]
            } else {
                let at = toks[term.name_at - 1].start;
                vec![Edit {
                    path: facts.path.to_owned(),
                    start: at,
                    end: at,
                    text: format!("->with('{rel}')"),
                }]
            };
            out.push(Finding {
                start: toks[j].start,
                end: name.end,
                message: format!(
                    "N+1 query: ${item}->{rel} runs one query per {}; eager load it with with('{rel}')",
                    model.name
                ),
                code: "laravel.n-plus-one",
                fixes: vec![Fix { title: format!("Eager load '{rel}' with with('{rel}')"), edits }],
                info: false,
            });
        }
    }
}

const BEFORE_LITERAL: &[&str] = &[
    "return", "yield", "echo", "print", "case", "else", "and", "or", "xor", "throw", "in", "as",
    "default", "clone",
];

fn opens_array(toks: &[Tok], i: usize) -> bool {
    let t = &toks[i];
    if t.kind == Kind::Ident && t.text.eq_ignore_ascii_case("array") {
        let after_arrow = i > 0
            && toks[i - 1].kind == Kind::Sym
            && matches!(toks[i - 1].text.as_str(), "->" | "?->" | "::");
        return !after_arrow && is_sym(toks.get(i + 1), "(");
    }
    if !(t.kind == Kind::Sym && t.text == "[") {
        return false;
    }
    let Some(prev) = i.checked_sub(1).map(|p| &toks[p]) else {
        return true;
    };
    match prev.kind {
        Kind::Var | Kind::Str | Kind::Num => false,
        Kind::Ident => BEFORE_LITERAL.contains(&prev.text.to_ascii_lowercase().as_str()),
        Kind::Sym => !matches!(prev.text.as_str(), ")" | "]" | "}" | "#"),
    }
}

fn entry_keys(toks: &[Tok], open: usize) -> Vec<(usize, String, usize)> {
    let close = closing(toks, open);
    let mut depth = 0;
    let mut out = Vec::new();
    let mut entry_start = true;
    let mut pending: Option<(usize, String)> = None;
    for i in open + 1..close {
        let t = &toks[i];
        if t.kind == Kind::Sym {
            match t.text.as_str() {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                "," if depth == 0 => {
                    if let Some((at, name)) = pending.take() {
                        out.push((at, name, i));
                    }
                    entry_start = true;
                    continue;
                }
                _ => {}
            }
        }
        if depth == 0 && entry_start && is_sym(toks.get(i + 1), "=>") {
            let key = match t.kind {
                Kind::Str if !t.text.contains('$') => Some(t.text.clone()),
                Kind::Num if !t.text.contains('.') => {
                    Some(t.text.trim_start_matches('0').to_owned())
                }
                _ => None,
            };
            pending = key.map(|k| (i, if k.is_empty() { "0".into() } else { k }));
        }
        entry_start = false;
    }
    if let Some((at, name)) = pending {
        out.push((at, name, close));
    }
    out
}

fn duplicate_keys(text: &str, toks: &[Tok], facts: &Facts, out: &mut Vec<Finding>) {
    let opens = (0..toks.len()).filter(|&i| opens_array(toks, i));
    for open in opens {
        let open = if toks[open].kind == Kind::Ident {
            open + 1
        } else {
            open
        };
        let keys = entry_keys(toks, open);
        for (n, (at, key, _)) in keys.iter().enumerate() {
            let Some((earlier, _, end)) = keys[..n].iter().rev().find(|(_, k, _)| k == key) else {
                continue;
            };
            let tok = &toks[*at];
            let line = text[..toks[*earlier].start].matches('\n').count() + 1;
            let shown = if tok.kind == Kind::Str {
                format!("'{key}'")
            } else {
                key.clone()
            };
            let remove_to = toks.get(end + 1).map_or(toks[*end].end, |t| t.start);
            let fixes = if toks[*end].text == "," {
                vec![Fix {
                    title: format!("Remove the overwritten {shown} entry"),
                    edits: vec![Edit {
                        path: facts.path.to_owned(),
                        start: toks[*earlier].start,
                        end: remove_to,
                        text: String::new(),
                    }],
                }]
            } else {
                Vec::new()
            };
            out.push(Finding {
                start: tok.start,
                end: tok.end,
                message: format!(
                    "Duplicate array key {shown}: it overwrites the value set on line {line}"
                ),
                code: "laravel.duplicate-key",
                fixes,
                info: false,
            });
        }
    }
}

pub fn php(text: &str, facts: &Facts) -> Vec<Finding> {
    let toks = tokenize(text);
    let mut out = Vec::new();
    if facts.enabled.columns {
        unknown_columns(&toks, facts, &mut out);
    }
    if facts.enabled.env {
        env_calls(&toks, facts, &mut out);
    }
    if facts.enabled.fillable {
        mass_assignment(&toks, facts, &mut out);
    }
    if facts.enabled.n_plus_one {
        n_plus_one(&toks, facts, &mut out);
    }
    if facts.enabled.duplicate_keys {
        duplicate_keys(text, &toks, facts, &mut out);
    }
    out.sort_by_key(|f| (f.start, f.end));
    out
}

fn env_entries(text: &str) -> Vec<(String, usize, usize, String)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let lead = line.len() - trimmed.len();
        let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let skip = trimmed.len() - body.len();
        if !body.starts_with('#')
            && let Some((key, value)) = body.split_once('=')
        {
            let key = key.trim();
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let start = at + lead + skip;
                out.push((
                    key.to_owned(),
                    start,
                    start + key.len(),
                    value.trim_end_matches(['\n', '\r']).to_owned(),
                ));
            }
        }
        at += line.len();
    }
    out
}

fn append(text: &str, line: &str) -> (usize, String) {
    let sep = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    (text.len(), format!("{sep}{line}\n"))
}

pub fn dotenv(
    path: &str,
    text: &str,
    other_path: &str,
    other: &str,
    example: bool,
) -> Vec<Finding> {
    let mine = env_entries(text);
    let theirs: BTreeSet<String> = env_entries(other).into_iter().map(|(k, ..)| k).collect();
    let other_name = other_path.rsplit(['/', '\\']).next().unwrap_or(other_path);
    let mut out = Vec::new();
    for (key, start, end, value) in mine {
        if theirs.contains(&key) {
            continue;
        }
        let (at, line) = if example {
            append(other, &format!("{key}={value}"))
        } else {
            append(other, &format!("{key}="))
        };
        let message = if example {
            format!(
                "{key} is in {} but not in {other_name}",
                path.rsplit(['/', '\\']).next().unwrap_or(path)
            )
        } else {
            format!("{key} is missing from {other_name}, so new checkouts won't know about it")
        };
        out.push(Finding {
            start,
            end,
            message,
            code: "laravel.env-keys",
            fixes: vec![Fix {
                title: format!("Add {key} to {other_name}"),
                edits: vec![Edit {
                    path: other_path.to_owned(),
                    start: at,
                    end: at,
                    text: line,
                }],
            }],
            info: example,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const POST: &str = "<?php\nnamespace App\\Models;\nclass Post extends Model\n{\n    protected $fillable = [\n        'title',\n        'body',\n    ];\n\n    public function user() { return $this->belongsTo(User::class); }\n}\n";

    fn facts<'a>(
        models: &'a [ModelFacts],
        columns: &'a BTreeMap<String, BTreeSet<String>>,
        config: &'a [(String, String)],
        rel: &'a str,
    ) -> Facts<'a> {
        Facts {
            path: "/p/app/Http/Controllers/A.php",
            rel_path: rel,
            models,
            columns,
            config_env: config,
            enabled: Enabled {
                columns: true,
                env: true,
                fillable: true,
                n_plus_one: true,
                duplicate_keys: true,
            },
        }
    }

    fn setup() -> Parts {
        let models = vec![ModelFacts {
            name: "Post".into(),
            table: "posts".into(),
            path: "/p/app/Models/Post.php".into(),
            source: POST.into(),
            relations: vec!["user".into(), "comments".into()],
        }];
        let mut columns = BTreeMap::new();
        columns.insert(
            "posts".to_owned(),
            [
                "id",
                "title",
                "body",
                "user_id",
                "published_at",
                "created_at",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        );
        let config = vec![("app.url".to_owned(), "APP_URL".to_owned())];
        (models, columns, config)
    }

    fn apply(text: &str, edit: &Edit) -> String {
        format!("{}{}{}", &text[..edit.start], edit.text, &text[edit.end..])
    }

    #[test]
    fn unknown_columns_suggest_the_closest_one() {
        let (m, c, e) = setup();
        let text = "<?php\n$p = Post::where('titel', 'x')->orderBy('published_at')->pluck('nope_at_all');\n";
        let found = php(text, &facts(&m, &c, &e, "app/A.php"));
        let codes: Vec<_> = found
            .iter()
            .map(|f| (&text[f.start..f.end], f.code))
            .collect();
        assert_eq!(
            codes,
            [
                ("titel", "laravel.unknown-column"),
                ("nope_at_all", "laravel.unknown-column")
            ]
        );
        assert!(
            found[0].message.ends_with("did you mean 'title'?"),
            "{}",
            found[0].message
        );
        assert_eq!(
            apply(text, &found[0].fixes[0].edits[0]),
            text.replace("'titel'", "'title'")
        );
        assert!(found[1].fixes.is_empty());
    }

    #[test]
    fn aliases_and_unknown_tables_are_not_checked() {
        let (m, c, e) = setup();
        let text = "<?php\nPost::selectRaw('count(*) as total')->orderBy('total')->get();\nDB::table('other')->where('zzz', 1)->get();\nPost::where('posts.title', 1)->get();\n";
        assert!(php(text, &facts(&m, &c, &e, "app/A.php")).is_empty());
    }

    #[test]
    fn env_outside_config_suggests_the_config_key() {
        let (m, c, e) = setup();
        let text = "<?php\n$url = env('APP_URL', 'http://localhost');\n$x = env('OTHER');\n$this->env('X');\n";
        let found = php(text, &facts(&m, &c, &e, "app/Services/A.php"));
        assert_eq!(found.len(), 2);
        assert_eq!(
            &text[found[0].start..found[0].end],
            "env('APP_URL', 'http://localhost')"
        );
        assert_eq!(
            apply(text, &found[0].fixes[0].edits[0]),
            text.replace("env('APP_URL', 'http://localhost')", "config('app.url')")
        );
        assert!(found[1].fixes.is_empty());
        assert!(php(text, &facts(&m, &c, &e, "config/app.php")).is_empty());
    }

    #[test]
    fn create_keys_outside_fillable_are_added_to_the_model() {
        let (m, c, e) = setup();
        let text = "<?php\nPost::create(['title' => 'a', 'user_id' => 1]);\n";
        let found = php(text, &facts(&m, &c, &e, "app/A.php"));
        assert_eq!(found.len(), 1);
        assert_eq!(&text[found[0].start..found[0].end], "user_id");
        let edit = &found[0].fixes[0].edits[0];
        assert_eq!(edit.path, "/p/app/Models/Post.php");
        assert!(
            apply(POST, edit)
                .contains("        'title',\n        'body',\n        'user_id',\n    ];"),
            "{}",
            apply(POST, edit)
        );
    }

    #[test]
    fn relation_access_in_a_loop_is_an_n_plus_one() {
        let (m, c, e) = setup();
        let text = "<?php\n$posts = Post::latest()->get();\nforeach ($posts as $post) {\n    echo $post->user->name;\n    echo $post->user->email;\n    echo $post->title;\n}\n";
        let found = php(text, &facts(&m, &c, &e, "app/A.php"));
        assert_eq!(found.len(), 1);
        assert_eq!(&text[found[0].start..found[0].end], "$post->user");
        assert_eq!(
            apply(text, &found[0].fixes[0].edits[0]),
            text.replace("latest()->get()", "latest()->with('user')->get()")
        );
        let loaded = "<?php\n$posts = Post::with('user')->get();\nforeach ($posts as $post) { echo $post->user->name; }\n";
        assert!(php(loaded, &facts(&m, &c, &e, "app/A.php")).is_empty());
        let all = "<?php\nforeach (Post::all() as $p) { $p->comments->count(); }\n";
        let found = php(all, &facts(&m, &c, &e, "app/A.php"));
        assert_eq!(
            apply(all, &found[0].fixes[0].edits[0]),
            all.replace("Post::all()", "Post::with('comments')->get()")
        );
    }

    #[test]
    fn repeated_array_keys_are_reported_with_a_fix() {
        let (m, c, e) = setup();
        let text = "<?php\n$product = Product::create([\n    'name' => $data['name'],\n    'stock' => $data['stock'],\n    'stock' => 1,\n    \"name\" => 'x',\n]);\n";
        let found = php(text, &facts(&m, &c, &e, "app/A.php"));
        let dupes: Vec<_> = found
            .iter()
            .filter(|f| f.code == "laravel.duplicate-key")
            .collect();
        assert_eq!(dupes.len(), 2, "{found:?}");
        assert_eq!(&text[dupes[0].start..dupes[0].end], "'stock'");
        assert_eq!(
            dupes[0].message,
            "Duplicate array key 'stock': it overwrites the value set on line 4"
        );
        assert_eq!(&text[dupes[1].start..dupes[1].end], "\"name\"");
        assert_eq!(
            apply(text, &dupes[0].fixes[0].edits[0]),
            text.replace("    'stock' => $data['stock'],\n", "")
        );
    }

    #[test]
    fn distinct_and_nested_keys_are_not_duplicates() {
        let (m, c, e) = setup();
        let text = "<?php\n$a = ['x' => ['k' => 1], 'y' => ['k' => 2], 1 => 'a', '1x' => 'b', \"$v\" => 1, \"$v\" => 2];\n$b = array('k' => 1, 'k' => 2);\n$c = $rows['k'] + $rows['k'];\n$d = [1, 1, 'a', 'a'];\n";
        let found = php(text, &facts(&m, &c, &e, "app/A.php"));
        let dupes: Vec<_> = found
            .iter()
            .filter(|f| f.code == "laravel.duplicate-key")
            .map(|f| (&text[f.start..f.end], f.start))
            .collect();
        assert_eq!(dupes.len(), 1, "{dupes:?}");
        assert!(dupes[0].1 > text.find("array(").unwrap());
    }

    #[test]
    fn env_files_are_compared_both_ways() {
        let env = "APP_NAME=Shop\nAPP_KEY=base64:secret\nSTRIPE_KEY=sk_live\n";
        let example = "APP_NAME=Laravel\nAPP_KEY=\nMAIL_HOST=127.0.0.1\n";
        let in_env = dotenv("/p/.env", env, "/p/.env.example", example, false);
        assert_eq!(in_env.len(), 1);
        assert!(!in_env[0].info);
        assert!(
            dotenv("/p/.env.example", example, "/p/.env", env, true)
                .iter()
                .all(|f| f.info)
        );
        assert_eq!(&env[in_env[0].start..in_env[0].end], "STRIPE_KEY");
        assert_eq!(
            apply(example, &in_env[0].fixes[0].edits[0]),
            format!("{example}STRIPE_KEY=\n")
        );
        let in_example = dotenv("/p/.env.example", example, "/p/.env", env, true);
        assert_eq!(
            &example[in_example[0].start..in_example[0].end],
            "MAIL_HOST"
        );
        assert_eq!(
            apply(env, &in_example[0].fixes[0].edits[0]),
            format!("{env}MAIL_HOST=127.0.0.1\n")
        );
    }
}
