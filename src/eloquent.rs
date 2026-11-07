use crate::php::{Kind, Token, tokenize};

const SINGLE: [&str; 16] = [
    "find",
    "findOrFail",
    "findOrNew",
    "first",
    "firstOrFail",
    "firstOrNew",
    "firstOrCreate",
    "firstWhere",
    "sole",
    "create",
    "forceCreate",
    "make",
    "updateOrCreate",
    "findSole",
    "fresh",
    "replicate",
];
const MANY: [&str; 9] = [
    "get",
    "all",
    "paginate",
    "simplePaginate",
    "cursorPaginate",
    "lazy",
    "cursor",
    "findMany",
    "hydrate",
];
const USER_CALLS: [&str; 5] = [
    "auth()->user()",
    "Auth::user()",
    "$request->user()",
    "request()->user()",
    "$this->user()",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Base {
    Var(String),
    This,
    AuthUser,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
    pub base: Base,
    pub path: Vec<String>,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Class {
        name: String,
        many: bool,
    },
    Chain {
        base: Base,
        path: Vec<String>,
        element: bool,
    },
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_before(text: &str, end: usize) -> usize {
    text[..end]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_word(*c))
        .map_or(0, |(i, c)| i + c.len_utf8())
}

fn arrow_before(text: &str, at: usize) -> Option<usize> {
    let before = &text[..at];
    before
        .strip_suffix("?->")
        .or_else(|| before.strip_suffix("->"))
        .map(str::len)
}

pub fn access_at(text: &str, cursor: usize, whole_word: bool) -> Option<Access> {
    let cursor = cursor.min(text.len());
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let start = word_before(text, cursor);
    let end = if whole_word {
        text[cursor..]
            .char_indices()
            .find(|(_, c)| !is_word(*c))
            .map_or(text.len(), |(i, _)| cursor + i)
    } else {
        cursor
    };
    if whole_word && start == end {
        return None;
    }
    let mut at = arrow_before(text, start)?;
    let mut path = Vec::new();
    loop {
        let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        let head = &text[line_start..at];
        if let Some(call) = USER_CALLS.iter().find(|c| head.ends_with(*c)) {
            let before = head.len() - call.len();
            let ok = before == 0
                || !head[..before]
                    .chars()
                    .next_back()
                    .is_some_and(|c| is_word(c) || c == '$' || c == '>' || c == ':');
            if ok || call.starts_with('$') {
                path.reverse();
                return Some(Access {
                    base: Base::AuthUser,
                    path,
                    start,
                    end,
                });
            }
        }
        let s = word_before(text, at);
        if s == at {
            return None;
        }
        let word = &text[s..at];
        if s > 0 && text[..s].ends_with('$') {
            path.reverse();
            let base = if word == "this" {
                Base::This
            } else {
                Base::Var(word.to_owned())
            };
            return Some(Access {
                base,
                path,
                start,
                end,
            });
        }
        path.push(word.to_owned());
        at = arrow_before(text, s)?;
    }
}

fn scope_start(text: &str, cursor: usize) -> usize {
    let before = &text[..cursor];
    let mut i = before.len();
    while let Some(found) = before[..i].rfind("function") {
        let ok_before = found == 0 || !is_word(before[..found].chars().next_back().unwrap_or(' '));
        let ok_after = !before[found + 8..].chars().next().is_some_and(is_word);
        if ok_before && ok_after {
            return found;
        }
        i = found;
    }
    0
}

fn ident(tokens: &[Token], i: usize) -> Option<&str> {
    match &tokens.get(i)?.kind {
        Kind::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

fn short(name: &str) -> String {
    name.trim_start_matches('?')
        .rsplit('\\')
        .next()
        .unwrap_or(name)
        .to_owned()
}

fn chain_from(tokens: &[Token], mut i: usize, end: usize) -> Option<(Base, Vec<String>)> {
    let base = match &tokens.get(i)?.kind {
        Kind::Var(v) if v == "this" => Base::This,
        Kind::Var(v) => Base::Var(v.clone()),
        _ => return None,
    };
    let mut path = Vec::new();
    i += 1;
    while i < end {
        if tokens[i].kind != Kind::ObjectOp {
            return None;
        }
        path.push(ident(tokens, i + 1)?.to_owned());
        i += 2;
    }
    Some((base, path))
}

fn statement_origin(tokens: &[Token], from: usize, end: usize) -> Option<Origin> {
    let first = tokens.get(from)?;
    if ident(tokens, from) == Some("new") {
        return Some(Origin::Class {
            name: short(ident(tokens, from + 1)?),
            many: false,
        });
    }
    if let Kind::Ident(class) = &first.kind
        && tokens.get(from + 1).map(|t| &t.kind) == Some(&Kind::DoubleColon)
    {
        let mut last = None;
        let mut depth = 0i32;
        for k in from + 2..end {
            match tokens[k].kind {
                Kind::Open(_) => depth += 1,
                Kind::Close(_) => depth -= 1,
                _ => {}
            }
            if depth == 0
                && let Some(name) = ident(tokens, k)
                && tokens.get(k + 1).map(|t| &t.kind) == Some(&Kind::Open('('))
                && matches!(
                    tokens.get(k - 1).map(|t| &t.kind),
                    Some(Kind::ObjectOp | Kind::DoubleColon)
                )
            {
                last = Some(name);
            }
        }
        let last = last?;
        let many = if SINGLE.contains(&last) {
            false
        } else if MANY.contains(&last) {
            true
        } else {
            return None;
        };
        return Some(Origin::Class {
            name: short(class),
            many,
        });
    }
    let (base, path) = chain_from(tokens, from, end)?;
    Some(Origin::Chain {
        base,
        path,
        element: false,
    })
}

fn docblock(text: &str, var: &str) -> Option<String> {
    let needle = format!("${var}");
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.find("@var ").map(|i| line[i + 5..].trim_start()) else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let (Some(class), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if name == needle && !class.contains(['|', '[', '<']) {
            found = Some(short(class));
        }
    }
    found
}

pub fn origin(text: &str, cursor: usize, var: &str) -> Option<Origin> {
    let cursor = cursor.min(text.len());
    let scope = scope_start(text, cursor);
    let tokens = tokenize(text, scope, cursor);
    let mut found = None;
    let is_var = |t: &Token| matches!(&t.kind, Kind::Var(v) if v == var);
    for (i, t) in tokens.iter().enumerate() {
        if !is_var(t) {
            continue;
        }
        if let Some(class) = ident(&tokens, i.wrapping_sub(1)).filter(|c| {
            c.trim_start_matches(['?', '\\'])
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
        }) && i > 0
        {
            found = Some(Origin::Class {
                name: short(class),
                many: false,
            });
            continue;
        }
        if tokens.get(i + 1).map(|t| &t.kind) == Some(&Kind::Assign) {
            let end = tokens[i + 2..]
                .iter()
                .position(|t| t.kind == Kind::Semicolon)
                .map_or(tokens.len(), |p| i + 2 + p);
            if let Some(o) = statement_origin(&tokens, i + 2, end) {
                found = Some(o);
            }
            continue;
        }
        if tokens.get(i + 1).map(|t| &t.kind) == Some(&Kind::DoubleArrow) {
            continue;
        }
        let as_at = if i >= 1 && ident(&tokens, i - 1) == Some("as") {
            Some(i - 1)
        } else if i >= 3
            && tokens[i - 1].kind == Kind::DoubleArrow
            && matches!(tokens[i - 2].kind, Kind::Var(_))
            && ident(&tokens, i - 3) == Some("as")
        {
            Some(i - 3)
        } else {
            None
        };
        let Some(as_at) = as_at else { continue };
        let Some(open) = tokens[..as_at]
            .iter()
            .rposition(|t| t.kind == Kind::Open('('))
        else {
            continue;
        };
        if open == 0 || ident(&tokens, open - 1) != Some("foreach") {
            continue;
        }
        if let Some((base, path)) = chain_from(&tokens, open + 1, as_at) {
            found = Some(Origin::Chain {
                base,
                path,
                element: true,
            });
        }
    }
    if let Some(class) = docblock(&text[scope..cursor], var) {
        return Some(Origin::Class {
            name: class,
            many: false,
        });
    }
    found
}

const RELATED_QUERIES: [&str; 9] = [
    "wherehas",
    "orwherehas",
    "wheredoesnthave",
    "orwheredoesnthave",
    "withwherehas",
    "has",
    "orhas",
    "doesnthave",
    "ordoesnthave",
];
const SAME_QUERIES: [&str; 7] = [
    "where",
    "orwhere",
    "wherenot",
    "orwherenot",
    "when",
    "unless",
    "tap",
];
const ELEMENT_CALLS: [&str; 20] = [
    "each",
    "map",
    "filter",
    "reject",
    "first",
    "last",
    "sortby",
    "sortbydesc",
    "every",
    "contains",
    "some",
    "flatmap",
    "groupby",
    "keyby",
    "partition",
    "sum",
    "mapwithkeys",
    "search",
    "unique",
    "doesntcontain",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureParam {
    pub head: String,
    pub steps: Vec<String>,
    pub related: Option<String>,
    pub builder: bool,
    pub at: usize,
}

fn enclosing_open(tokens: &[Token], before: usize) -> Option<usize> {
    let mut depth = 0usize;
    for j in (0..before).rev() {
        match tokens[j].kind {
            Kind::Close(_) => depth += 1,
            Kind::Open(_) if depth == 0 => return Some(j),
            Kind::Open(_) => depth -= 1,
            _ => {}
        }
    }
    None
}

fn still_open(tokens: &[Token], open: usize, from: usize) -> bool {
    let mut depth = 0usize;
    for (j, t) in tokens.iter().enumerate().skip(open + 1) {
        match t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) if depth == 0 => return false,
            Kind::Close(_) => depth -= 1,
            Kind::Comma if depth == 0 && j > from => return false,
            _ => {}
        }
    }
    true
}

fn param_index(tokens: &[Token], open: usize, var: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = 0;
    for t in &tokens[open + 1..] {
        match &t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) if depth == 0 => return None,
            Kind::Close(_) => depth -= 1,
            Kind::Comma if depth == 0 => index += 1,
            Kind::Var(v) if depth == 0 && v == var => return Some(index),
            _ => {}
        }
    }
    None
}

pub fn closure_param(text: &str, cursor: usize, var: &str) -> Option<ClosureParam> {
    let cursor = cursor.min(text.len());
    let tokens = tokenize(text, 0, cursor);
    for k in (0..tokens.len()).rev() {
        let keyword = ident(&tokens, k)
            .is_some_and(|w| w.eq_ignore_ascii_case("fn") || w.eq_ignore_ascii_case("function"));
        if !keyword || tokens.get(k + 1).map(|t| &t.kind) != Some(&Kind::Open('(')) {
            continue;
        }
        let Some(param) = param_index(&tokens, k + 1, var) else {
            continue;
        };
        let Some(mut open) = enclosing_open(&tokens, k) else {
            continue;
        };
        let mut key = None;
        if tokens[open].kind == Kind::Open('[') {
            if !still_open(&tokens, open, k) {
                continue;
            }
            if k >= 2
                && tokens[k - 1].kind == Kind::DoubleArrow
                && let Kind::Str { value, .. } = &tokens[k - 2].kind
            {
                key = Some(value.clone());
            }
            let Some(outer) = enclosing_open(&tokens, open) else {
                continue;
            };
            open = outer;
        }
        if tokens[open].kind != Kind::Open('(') || !still_open(&tokens, open, k) {
            continue;
        }
        let Some(name) = ident(&tokens, open.wrapping_sub(1)).map(str::to_ascii_lowercase) else {
            continue;
        };
        let Some((head, steps)) = crate::php::chain(&tokens, open) else {
            continue;
        };
        let at = tokens[k].start;
        let (related, builder) = if let Some(key) = key {
            (Some(key), true)
        } else if RELATED_QUERIES.contains(&name.as_str()) {
            match &tokens.get(open + 1).map(|t| &t.kind) {
                Some(Kind::Str { value, .. }) if tokens[open + 2].start < tokens[k].start => {
                    (Some(value.clone()), true)
                }
                _ => continue,
            }
        } else if SAME_QUERIES.contains(&name.as_str()) {
            (None, true)
        } else if ELEMENT_CALLS.contains(&name.as_str()) {
            (None, false)
        } else {
            continue;
        };
        if param != 0 {
            continue;
        }
        return Some(ClosureParam {
            head,
            steps,
            related,
            builder,
            at,
        });
    }
    None
}

pub fn is_many(kind: &str) -> bool {
    matches!(
        kind,
        "hasMany"
            | "belongsToMany"
            | "morphMany"
            | "morphToMany"
            | "morphedByMany"
            | "hasManyThrough"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(text: &str) -> Option<(Base, Vec<String>, String)> {
        let cursor = text.find('|').unwrap();
        let text = text.replace('|', "");
        let a = access_at(&text, cursor, false)?;
        Some((a.base, a.path, text[a.start..a.end].to_owned()))
    }

    #[test]
    fn reads_member_accesses() {
        assert_eq!(
            access("$order->|"),
            Some((Base::Var("order".into()), vec![], String::new()))
        );
        assert_eq!(
            access("echo $order->user?->na|"),
            Some((Base::Var("order".into()), vec!["user".into()], "na".into()))
        );
        assert_eq!(
            access("$this->items->|"),
            Some((Base::This, vec!["items".into()], String::new()))
        );
        assert_eq!(
            access("auth()->user()->team->|"),
            Some((Base::AuthUser, vec!["team".into()], String::new()))
        );
        assert_eq!(access("$order->items()->|"), None);
        assert_eq!(access("Order::|"), None);
    }

    #[test]
    fn whole_words_for_navigation() {
        let text = "$order->user->name;";
        let a = access_at(text, text.find("user").unwrap() + 2, true).unwrap();
        assert_eq!(
            (a.base, a.path, &text[a.start..a.end]),
            (Base::Var("order".into()), vec![], "user")
        );
    }

    #[test]
    fn infers_where_variables_come_from() {
        let text = "<?php\nclass C {\n    public function store(PlaceOrderRequest $request, ?Shop $shop)\n    {\n        $order = Order::create(['user_id' => $request->user()->id, 'total' => 0]);\n        $list = Order::query()->where('a', 1)->get();\n        $builder = Order::query()->where('a', 1);\n        $user = $order->user;\n        foreach ($order->items as $item) {}\n        foreach ($list as $key => $row) {}\n        /** @var \\App\\Models\\Product $product */\n        $product = app('x');\n        $fresh = new \\App\\Models\\Review();\n        |\n    }\n}\n";
        let cursor = text.find('|').unwrap();
        let text = text.replace('|', "");
        let class = |name: &str, many: bool| {
            Some(Origin::Class {
                name: name.into(),
                many,
            })
        };
        assert_eq!(origin(&text, cursor, "order"), class("Order", false));
        assert_eq!(origin(&text, cursor, "list"), class("Order", true));
        assert_eq!(origin(&text, cursor, "builder"), None);
        assert_eq!(origin(&text, cursor, "shop"), class("Shop", false));
        assert_eq!(
            origin(&text, cursor, "request"),
            class("PlaceOrderRequest", false)
        );
        assert_eq!(
            origin(&text, cursor, "user"),
            Some(Origin::Chain {
                base: Base::Var("order".into()),
                path: vec!["user".into()],
                element: false
            })
        );
        assert_eq!(
            origin(&text, cursor, "item"),
            Some(Origin::Chain {
                base: Base::Var("order".into()),
                path: vec!["items".into()],
                element: true
            })
        );
        assert_eq!(
            origin(&text, cursor, "row"),
            Some(Origin::Chain {
                base: Base::Var("list".into()),
                path: vec![],
                element: true
            })
        );
        assert_eq!(origin(&text, cursor, "product"), class("Product", false));
        assert_eq!(origin(&text, cursor, "fresh"), class("Review", false));
    }
}
