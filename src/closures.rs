use crate::php::{Kind, tokenize};
use crate::relations::Shape;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callee {
    Member { name: String, name_end: usize },
    Static { class: String, name: String },
    Function { name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub callee: Callee,
    pub arg: usize,
    pub start: usize,
    pub typed: String,
    pub first_class: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub class: Option<String>,
    pub fqn: Option<String>,
    pub var: String,
}

impl Param {
    fn typed(class: &str, fqn: Option<&str>, var: &str) -> Self {
        Self {
            class: Some(class.to_owned()),
            fqn: fqn.map(str::to_owned),
            var: var.to_owned(),
        }
    }

    fn plain(var: &str) -> Self {
        Self {
            class: None,
            fqn: None,
            var: var.to_owned(),
        }
    }

    fn array(var: &str) -> Self {
        Self::typed("array", None, var)
    }
}

pub const BUILDER: &str = "Illuminate\\Database\\Eloquent\\Builder";
const COLLECTION: &str = "Illuminate\\Database\\Eloquent\\Collection";
const BLUEPRINT: &str = "Illuminate\\Database\\Schema\\Blueprint";
const REQUEST: &str = "Illuminate\\Http\\Request";
const SEQUENCE: &str = "Illuminate\\Database\\Eloquent\\Factories\\Sequence";
const QUERY_EXECUTED: &str = "Illuminate\\Database\\Events\\QueryExecuted";
const VALIDATOR: &str = "Illuminate\\Validation\\Validator";

const EACH_ITEM: &[&str] = &[
    "map",
    "each",
    "filter",
    "reject",
    "first",
    "last",
    "sortBy",
    "sortByDesc",
    "groupBy",
    "keyBy",
    "mapWithKeys",
    "flatMap",
    "sum",
    "avg",
    "max",
    "min",
    "contains",
    "every",
    "partition",
    "unique",
    "transform",
    "search",
    "skipUntil",
    "skipWhile",
    "takeUntil",
    "takeWhile",
    "doesntContain",
    "sole",
    "firstOrFail",
    "mapToGroups",
    "countBy",
];

const MODEL_EVENTS: &[&str] = &[
    "retrieved",
    "creating",
    "created",
    "updating",
    "updated",
    "saving",
    "saved",
    "deleting",
    "deleted",
    "restoring",
    "restored",
    "replicating",
    "forceDeleting",
    "forceDeleted",
    "trashed",
];

pub fn var_name(class: &str) -> String {
    let mut chars = class.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
}

fn plural(word: &str) -> String {
    if word.ends_with('y') && !word.ends_with("ey") {
        format!("{}ies", &word[..word.len() - 1])
    } else if word.ends_with('s') {
        format!("{word}es")
    } else {
        format!("{word}s")
    }
}

pub fn site(text: &str, cursor: usize) -> Option<Site> {
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let start = text[..cursor]
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let typed = &text[start..cursor];
    if !("fn".starts_with(typed) || "function".starts_with(typed)) {
        return None;
    }
    let before = text[..start].trim_end();
    if !before.ends_with(['(', ',']) {
        return None;
    }
    let tokens = tokenize(text, 0, start);
    let mut depth = 0usize;
    let mut arg = 0usize;
    let mut paren = None;
    for (i, t) in tokens.iter().enumerate().rev() {
        match t.kind {
            Kind::Close(_) => depth += 1,
            Kind::Open(c) if depth == 0 => {
                if c != '(' {
                    return None;
                }
                paren = Some(i);
                break;
            }
            Kind::Open(_) => depth -= 1,
            Kind::Comma if depth == 0 => arg += 1,
            Kind::Semicolon if depth == 0 => return None,
            _ => {}
        }
    }
    let paren = paren?;
    let name_token = tokens.get(paren.checked_sub(1)?)?;
    let Kind::Ident(name) = &name_token.kind else {
        return None;
    };
    let name = name.rsplit('\\').next().unwrap_or(name).to_owned();
    let callee = match paren
        .checked_sub(2)
        .and_then(|k| tokens.get(k))
        .map(|t| &t.kind)
    {
        Some(Kind::ObjectOp) => Callee::Member {
            name,
            name_end: name_token.end,
        },
        Some(Kind::DoubleColon) => {
            let Some(Kind::Ident(class)) = paren
                .checked_sub(3)
                .and_then(|k| tokens.get(k))
                .map(|t| &t.kind)
            else {
                return None;
            };
            Callee::Static {
                class: class.trim_start_matches('\\').to_owned(),
                name,
            }
        }
        Some(Kind::Ident(word)) if word == "function" || word == "fn" => return None,
        _ => Callee::Function { name },
    };
    let first_class = match (
        tokens.get(paren + 1),
        tokens.get(paren + 2),
        tokens.get(paren + 3),
    ) {
        (Some(a), Some(b), Some(c))
            if b.kind == Kind::DoubleColon && matches!(&c.kind, Kind::Ident(w) if w == "class") =>
        {
            match &a.kind {
                Kind::Ident(class) => Some(class.trim_start_matches('\\').to_owned()),
                _ => None,
            }
        }
        _ => None,
    };
    Some(Site {
        callee,
        arg,
        start,
        typed: typed.to_owned(),
        first_class,
    })
}

pub struct Receiver<'a> {
    pub model: &'a str,
    pub model_fqn: Option<&'a str>,
    pub shape: Shape,
    pub is_static: bool,
}

pub fn params(
    site: &Site,
    receiver: Option<&Receiver>,
    user: Option<(&str, &str)>,
) -> Option<Vec<Param>> {
    let model_param = |r: &Receiver| Param::typed(r.model, r.model_fqn, &var_name(r.model));
    let builder = || Param::typed("Builder", Some(BUILDER), "query");
    let none = Vec::new;
    match &site.callee {
        Callee::Static { class, name } => {
            let class = class.rsplit('\\').next().unwrap_or(class);
            if let Some(r) = receiver.filter(|r| r.is_static)
                && MODEL_EVENTS.contains(&name.as_str())
                && site.arg == 0
            {
                return Some(vec![model_param(r)]);
            }
            let found = match (class, name.as_str(), site.arg) {
                ("DB", "transaction", 0) => none(),
                ("DB", "listen", 0) => {
                    vec![Param::typed("QueryExecuted", Some(QUERY_EXECUTED), "query")]
                }
                ("DB", "afterCommit", 0) => none(),
                ("Schema", "create" | "table" | "connection", 1) => {
                    vec![Param::typed("Blueprint", Some(BLUEPRINT), "table")]
                }
                ("Route", "group", 0 | 1) => none(),
                ("Route", "bind", 1) => vec![Param::typed("string", None, "value")],
                ("Cache", "remember" | "flexible", 2) => none(),
                ("Cache", "rememberForever" | "sear", 1) => none(),
                ("Gate", "define" | "before" | "after", _) => match user {
                    Some((class, fqn)) => vec![Param::typed(class, Some(fqn), &var_name(class))],
                    None => vec![Param::plain("user")],
                },
                ("Event", "listen", 1) => match &site.first_class {
                    Some(event) => {
                        let short = event.rsplit('\\').next().unwrap_or(event);
                        vec![Param::typed(short, None, "event")]
                    }
                    None => vec![Param::plain("event")],
                },
                ("RateLimiter", "for", 1) => {
                    vec![Param::typed("Request", Some(REQUEST), "request")]
                }
                ("Blade", "directive", 1) => vec![Param::typed("string", None, "expression")],
                ("Blade", "if", 1) => vec![Param::plain("value")],
                ("Validator", "extend", 1) => vec![
                    Param::plain("attribute"),
                    Param::plain("value"),
                    Param::array("parameters"),
                ],
                ("Queue", "before" | "after" | "failing", 0) => vec![Param::plain("event")],
                ("Schedule", "call", 0) => none(),
                ("Http", "fake", 0) => vec![Param::typed(
                    "Request",
                    Some("Illuminate\\Http\\Client\\Request"),
                    "request",
                )],
                _ => return receiver.and_then(|r| member_params(name, site.arg, r)),
            };
            Some(found)
        }
        Callee::Member { name, .. } => {
            if name == "group" && site.arg == 0 && receiver.is_none() {
                return Some(none());
            }
            if name == "after" && site.arg == 0 && receiver.is_none() {
                return Some(vec![Param::typed(
                    "Validator",
                    Some(VALIDATOR),
                    "validator",
                )]);
            }
            let r = receiver?;
            if matches!(
                name.as_str(),
                "where" | "orWhere" | "whereNot" | "orWhereNot"
            ) && site.arg == 0
                && r.shape == Shape::Query
            {
                return Some(vec![builder()]);
            }
            member_params(name, site.arg, r)
        }
        Callee::Function { name } => match (name.as_str(), site.arg) {
            ("tap", 1)
            | ("retry", 1)
            | ("rescue", 0)
            | ("once", 0)
            | ("defer", 0)
            | ("dispatch", 0) => Some(none()),
            ("value", 0) | ("with", 1) => Some(none()),
            _ => None,
        },
    }
}

fn member_params(name: &str, arg: usize, r: &Receiver) -> Option<Vec<Param>> {
    let model = || Param::typed(r.model, r.model_fqn, &var_name(r.model));
    let builder = || Param::typed("Builder", Some(BUILDER), "query");
    let models = || Param::typed("Collection", Some(COLLECTION), &plural(&var_name(r.model)));
    match (r.shape, name, arg) {
        (Shape::Many, "reduce", 0) => Some(vec![Param::plain("carry"), model()]),
        (Shape::Many, "sort", 0) => Some(vec![
            Param::typed(r.model, r.model_fqn, "a"),
            Param::typed(r.model, r.model_fqn, "b"),
        ]),
        (Shape::Many, "when" | "unless", 1) => Some(vec![models(), Param::plain("value")]),
        (Shape::Many, "pipe" | "tap", 0) => Some(vec![models()]),
        (Shape::Many, n, 0) if EACH_ITEM.contains(&n) => Some(vec![model()]),
        (Shape::Query, "where" | "orWhere" | "whereNot" | "orWhereNot", 0) => Some(vec![builder()]),
        (
            Shape::Query | Shape::Model,
            "whereHas" | "orWhereHas" | "whereDoesntHave" | "orWhereDoesntHave" | "withWhereHas"
            | "whereHasMorph",
            1,
        ) => Some(vec![builder()]),
        (Shape::Query, "when" | "unless", 1 | 2) => Some(vec![builder(), Param::plain("value")]),
        (Shape::Query, "tap", 0) => Some(vec![builder()]),
        (Shape::Query, "each", 0) => Some(vec![model()]),
        (Shape::Query, "chunk" | "chunkById" | "chunkByIdDesc", 1) => Some(vec![models()]),
        (Shape::Query, "lazy" | "cursor", 0) => None,
        (Shape::Query, "firstOr" | "sole", _) => Some(Vec::new()),
        (Shape::Factory, "afterCreating" | "afterMaking", 0) => Some(vec![model()]),
        (Shape::Factory, "state", 0) => Some(vec![Param::array("attributes")]),
        (Shape::Factory, "sequence", _) => {
            Some(vec![Param::typed("Sequence", Some(SEQUENCE), "sequence")])
        }
        (Shape::Model, "load" | "loadMissing", 0) => None,
        _ => None,
    }
}

pub fn signature(params: &[Param]) -> (String, String) {
    let shown: Vec<String> = params
        .iter()
        .map(|p| match &p.class {
            Some(class) => format!("{class} ${}", p.var),
            None => format!("${}", p.var),
        })
        .collect();
    let escaped: Vec<String> = params
        .iter()
        .map(|p| match &p.class {
            Some(class) => format!("{class} \\${}", p.var),
            None => format!("\\${}", p.var),
        })
        .collect();
    (shown.join(", "), escaped.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Option<Site> {
        site(text, text.len())
    }

    #[test]
    fn finds_the_call_and_argument() {
        let s = at("<?php\n$users->map(fn").unwrap();
        assert_eq!(
            s.callee,
            Callee::Member {
                name: "map".into(),
                name_end: 17
            }
        );
        assert_eq!((s.arg, s.typed.as_str()), (0, "fn"));
        let s = at("<?php\nUser::whereHas('posts', ").unwrap();
        assert_eq!(s.arg, 1);
        assert!(
            matches!(s.callee, Callee::Static { ref class, ref name } if class == "User" && name == "whereHas")
        );
        let s = at("<?php\nEvent::listen(OrderShipped::class, f").unwrap();
        assert_eq!(s.first_class.as_deref(), Some("OrderShipped"));
        assert_eq!(at("<?php\n$users->map($x"), None);
        assert_eq!(at("<?php\nfoo(['a' => fn"), None);
        assert_eq!(at("<?php\n$x = fn"), None);
    }

    #[test]
    fn picks_parameter_types() {
        let users = Receiver {
            model: "User",
            model_fqn: Some("App\\Models\\User"),
            shape: Shape::Many,
            is_static: false,
        };
        let map = at("<?php\n$users->map(").unwrap();
        let p = params(&map, Some(&users), None).unwrap();
        assert_eq!(signature(&p).0, "User $user");
        let query = Receiver {
            shape: Shape::Query,
            ..users
        };
        let chunk = at("<?php\nUser::query()->chunk(100, ").unwrap();
        assert_eq!(
            signature(&params(&chunk, Some(&query), None).unwrap()).0,
            "Collection $users"
        );
        let schema = at("<?php\nSchema::create('posts', ").unwrap();
        assert_eq!(
            signature(&params(&schema, None, None).unwrap()).0,
            "Blueprint $table"
        );
        let transaction = at("<?php\nDB::transaction(").unwrap();
        assert_eq!(params(&transaction, None, None).unwrap(), Vec::new());
        let creating = at("<?php\nstatic::creating(").unwrap();
        let model = Receiver {
            shape: Shape::Query,
            is_static: true,
            ..users
        };
        assert_eq!(
            signature(&params(&creating, Some(&model), None).unwrap()).0,
            "User $user"
        );
    }
}
