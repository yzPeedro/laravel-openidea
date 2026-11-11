use crate::checks::{Edit, Finding, Fix};
use crate::querysql::{Kind, Tok, tokenize};

const MASS_METHODS: &[&str] = &[
    "create",
    "createQuietly",
    "forceCreate",
    "forceCreateQuietly",
    "update",
    "updateQuietly",
    "fill",
    "forceFill",
    "insert",
    "insertOrIgnore",
    "updateOrCreate",
    "firstOrCreate",
    "firstOrNew",
    "createOrFirst",
    "make",
    "updateOrInsert",
    "upsert",
];

const RAW_METHODS: &[&str] = &[
    "whereRaw",
    "orWhereRaw",
    "havingRaw",
    "orHavingRaw",
    "orderByRaw",
    "groupByRaw",
    "selectRaw",
    "fromRaw",
];

const RAW_STATICS: &[&str] = &[
    "raw",
    "select",
    "selectOne",
    "scalar",
    "statement",
    "unprepared",
    "insert",
    "update",
    "delete",
    "affectingStatement",
];

const REQUEST_IN_BLADE: &[&str] = &[
    "request(",
    "$request",
    "Request::",
    "old(",
    "$_GET",
    "$_POST",
    "$_REQUEST",
    "$_COOKIE",
    "->input(",
    "->query(",
];

fn sym(t: Option<&Tok>, s: &str) -> bool {
    t.is_some_and(|t| t.kind == Kind::Sym && t.text == s)
}

fn ident(t: Option<&Tok>, s: &str) -> bool {
    t.is_some_and(|t| t.kind == Kind::Ident && t.text.eq_ignore_ascii_case(s))
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

fn form_request_vars(toks: &[Tok]) -> Vec<String> {
    toks.windows(2)
        .filter_map(|w| {
            let short = w[0].text.rsplit('\\').next().unwrap_or(&w[0].text);
            (w[0].kind == Kind::Ident
                && w[1].kind == Kind::Var
                && short.ends_with("Request")
                && short != "Request")
                .then(|| w[1].text.clone())
        })
        .collect()
}

fn request_vars(toks: &[Tok]) -> Vec<String> {
    toks.windows(2)
        .filter_map(|w| {
            let short = w[0].text.rsplit('\\').next().unwrap_or(&w[0].text);
            (w[0].kind == Kind::Ident && w[1].kind == Kind::Var && short.ends_with("Request"))
                .then(|| w[1].text.clone())
        })
        .collect()
}

struct AllInput {
    start: usize,
    method_at: usize,
    form_request: bool,
}

fn all_input_at(toks: &[Tok], i: usize, requests: &[String], forms: &[String]) -> Option<AllInput> {
    let no_args = |at: usize| sym(toks.get(at), "(") && sym(toks.get(at + 1), ")");
    let method = |at: usize| {
        let t = toks.get(at)?;
        (t.kind == Kind::Ident && (t.text == "all" || t.text == "input") && no_args(at + 1))
            .then_some(at)
    };
    let t = &toks[i];
    if t.kind == Kind::Var && sym(toks.get(i + 1), "->") {
        let name = &t.text;
        if !(requests.contains(name) || name.to_ascii_lowercase().contains("request")) {
            return None;
        }
        let at = method(i + 2)?;
        return Some(AllInput {
            start: i,
            method_at: at,
            form_request: forms.contains(name),
        });
    }
    if ident(Some(t), "request")
        && no_args(i + 1)
        && sym(toks.get(i + 3), "->")
        && !sym(toks.get(i.wrapping_sub(1)), "->")
    {
        let at = method(i + 4)?;
        return Some(AllInput {
            start: i,
            method_at: at,
            form_request: false,
        });
    }
    let short = t.text.rsplit('\\').next().unwrap_or(&t.text);
    if t.kind == Kind::Ident
        && (short == "Request" || short == "Input")
        && sym(toks.get(i + 1), "::")
    {
        let at = method(i + 2)?;
        return Some(AllInput {
            start: i,
            method_at: at,
            form_request: false,
        });
    }
    None
}

fn enclosing_call(toks: &[Tok], from: usize) -> Option<(usize, String)> {
    let mut depth = 0i32;
    let mut k = from;
    while k > 0 {
        k -= 1;
        let t = &toks[k];
        if t.kind != Kind::Sym {
            continue;
        }
        match t.text.as_str() {
            ")" | "]" | "}" => depth += 1,
            "[" if depth == 0 => {}
            "(" if depth == 0 => {
                let name = toks.get(k.checked_sub(1)?)?;
                if name.kind != Kind::Ident {
                    return None;
                }
                let before = k.checked_sub(2).and_then(|b| toks.get(b));
                let callable = sym(before, "->")
                    || sym(before, "?->")
                    || sym(before, "::")
                    || ident(before, "new");
                return callable.then(|| {
                    let short = name.text.rsplit('\\').next().unwrap_or(&name.text);
                    let label = if ident(before, "new") {
                        format!("new {short}()")
                    } else {
                        format!("{short}()")
                    };
                    (k, label)
                });
            }
            "(" | "[" | "{" => depth -= 1,
            ";" if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

fn mass_input(toks: &[Tok], path: &str, out: &mut Vec<Finding>) {
    let forms = form_request_vars(toks);
    let requests = request_vars(toks);
    for i in 0..toks.len() {
        let Some(found) = all_input_at(toks, i, &requests, &forms) else {
            continue;
        };
        let Some((open, label)) = enclosing_call(toks, found.start) else {
            continue;
        };
        let callee = &toks[open - 1].text;
        let constructor = label.starts_with("new ");
        if !constructor && !MASS_METHODS.contains(&callee.as_str()) {
            continue;
        }
        let end = toks[found.method_at + 2].end;
        let method = &toks[found.method_at];
        let mut fixes = Vec::new();
        if found.form_request {
            fixes.push(Fix {
                title: "Use validated()".into(),
                edits: vec![Edit {
                    path: path.to_owned(),
                    start: method.start,
                    end: method.end,
                    text: "validated".into(),
                }],
            });
        }
        out.push(Finding {
            start: toks[found.start].start,
            end,
            message: format!(
                "Every input field goes to {label}: pass $request->validated() or $request->only([...]) instead"
            ),
            code: "laravel.mass-assign-input",
            fixes,
            info: false,
        });
    }
}

fn interpolated(src: &str, t: &Tok) -> bool {
    if t.kind != Kind::Str || !src[t.start..].starts_with('"') {
        return false;
    }
    let b = t.text.as_bytes();
    b.iter().enumerate().any(|(i, &c)| {
        c == b'$'
            && (i == 0 || b[i - 1] != b'\\')
            && b.get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == b'_')
    })
}

fn raw_sql(src: &str, toks: &[Tok], out: &mut Vec<Finding>) {
    for i in 1..toks.len() {
        let t = &toks[i];
        if t.kind != Kind::Ident || !sym(toks.get(i + 1), "(") {
            continue;
        }
        let before = &toks[i - 1];
        let raw_method = (sym(Some(before), "->") || sym(Some(before), "?->"))
            && RAW_METHODS.contains(&t.text.as_str());
        let raw_static = sym(Some(before), "::")
            && i >= 2
            && toks[i - 2].text.rsplit('\\').next() == Some("DB")
            && RAW_STATICS.contains(&t.text.as_str());
        if !raw_method && !raw_static {
            continue;
        }
        let open = i + 1;
        let close = closing(toks, open);
        let mut depth = 0i32;
        let mut end_arg = close;
        for (k, a) in toks.iter().enumerate().take(close).skip(open + 1) {
            if a.kind == Kind::Sym {
                match a.text.as_str() {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    "," if depth == 0 => {
                        end_arg = k;
                        break;
                    }
                    _ => {}
                }
            }
        }
        let arg = &toks[open + 1..end_arg];
        let Some(first) = arg.first() else { continue };
        let variable = arg
            .iter()
            .any(|a| (a.kind == Kind::Var && a.text != "this") || interpolated(src, a));
        if !variable {
            continue;
        }
        let last = arg.last().unwrap_or(first);
        let name = if raw_static {
            format!("DB::{}()", t.text)
        } else {
            format!("{}()", t.text)
        };
        out.push(Finding {
            start: first.start,
            end: last.end,
            message: format!(
                "Variable in the SQL of {name}: pass values as bindings (?) so they are escaped"
            ),
            code: "laravel.raw-sql-variable",
            fixes: Vec::new(),
            info: false,
        });
    }
}

pub fn php(text: &str, path: &str) -> Vec<Finding> {
    let toks = tokenize(text);
    let mut out = Vec::new();
    mass_input(&toks, path, &mut out);
    raw_sql(text, &toks, &mut out);
    out
}

pub fn blade(text: &str, path: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find("{!!") {
        let open = at + found;
        if text[..open].ends_with('@') {
            at = open + 3;
            continue;
        }
        let Some(len) = text[open + 3..].find("!!}") else {
            break;
        };
        let close = open + 3 + len;
        let expr = &text[open + 3..close];
        at = close + 3;
        if !REQUEST_IN_BLADE.iter().any(|n| expr.contains(n)) {
            continue;
        }
        let lead = expr.len() - expr.trim_start().len();
        let start = open + 3 + lead;
        let end = open + 3 + expr.trim_end().len();
        out.push(Finding {
            start,
            end,
            message: "Request data printed without escaping: anyone can inject HTML or scripts here. Use {{ }} to escape it".into(),
            code: "laravel.unescaped-request",
            fixes: vec![Fix {
                title: "Escape with {{ }}".into(),
                edits: vec![
                    Edit {
                        path: path.to_owned(),
                        start: open,
                        end: open + 3,
                        text: "{{".into(),
                    },
                    Edit {
                        path: path.to_owned(),
                        start: close,
                        end: close + 3,
                        text: "}}".into(),
                    },
                ],
            }],
            info: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(found: &[Finding], text: &str) -> Vec<String> {
        found
            .iter()
            .map(|f| format!("{}:{}", f.code, &text[f.start..f.end]))
            .collect()
    }

    #[test]
    fn flags_every_input_field_passed_to_mass_assignment() {
        let text = "<?php\nclass OrderController\n{\n    public function store(StoreOrderRequest $request, Request $plain)\n    {\n        Order::create($request->all());\n        $order->update(request()->all());\n        $order->fill([...$plain->input(), 'user_id' => 1]);\n        new Order(Request::all());\n        Order::create($request->validated());\n        Order::create($request->only(['total']));\n        $items = collect($request->all());\n        $data->all();\n    }\n}\n";
        let found = php(text, "/p/app/Http/Controllers/OrderController.php");
        assert_eq!(
            codes(&found, text),
            [
                "laravel.mass-assign-input:$request->all()",
                "laravel.mass-assign-input:request()->all()",
                "laravel.mass-assign-input:$plain->input()",
                "laravel.mass-assign-input:Request::all()",
            ]
        );
        assert!(found[0].message.contains("create()"));
        assert!(found[3].message.contains("new Order()"));
        assert_eq!(found[0].fixes[0].title, "Use validated()");
        assert_eq!(found[0].fixes[0].edits[0].text, "validated");
        assert!(found[1].fixes.is_empty());
        assert!(found[2].fixes.is_empty());
    }

    #[test]
    fn flags_variables_in_raw_sql() {
        let text = "<?php\n$q->whereRaw(\"status = '$status'\");\n$q->orderByRaw('total ' . $direction);\nDB::select(\"select * from users where id = {$id}\");\nDB::raw('count(*) as total');\n$q->whereRaw('status = ?', [$status]);\nDB::statement('drop table ' . $this->table);\n$q->selectRaw(\"price \\$total\");\n";
        let found = php(text, "/p/app/X.php");
        assert_eq!(
            codes(&found, text),
            [
                "laravel.raw-sql-variable:\"status = '$status'\"",
                "laravel.raw-sql-variable:'total ' . $direction",
                "laravel.raw-sql-variable:\"select * from users where id = {$id}\"",
            ]
        );
        assert!(found[0].message.contains("whereRaw()"));
        assert!(found[2].message.contains("DB::select()"));
    }

    #[test]
    fn flags_unescaped_request_data_in_blade() {
        let text = "<p>{!! request('q') !!}</p>\n<p>{!! $post->body !!}</p>\n<p>{!! old('name') !!}</p>\n@{!! request('x') !!}\n<p>{{ request('q') }}</p>\n";
        let found = blade(text, "/p/resources/views/x.blade.php");
        assert_eq!(
            codes(&found, text),
            [
                "laravel.unescaped-request:request('q')",
                "laravel.unescaped-request:old('name')",
            ]
        );
        let edits = &found[0].fixes[0].edits;
        assert_eq!((edits[0].start, edits[0].text.as_str()), (3, "{{"));
        assert_eq!(&text[edits[1].start..edits[1].end], "!!}");
    }
}
