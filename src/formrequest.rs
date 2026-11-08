use crate::querysql::{Kind, Tok, tokenize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extraction {
    pub class: String,
    pub file: String,
    pub content: String,
    pub edits: Vec<(usize, usize, String)>,
}

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

fn args(toks: &[Tok], open: usize, close: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = open + 1;
    let mut i = open + 1;
    while i < close {
        if sym(toks.get(i), "(") || sym(toks.get(i), "[") || sym(toks.get(i), "{") {
            i = closing(toks, i) + 1;
            continue;
        }
        if sym(toks.get(i), ",") {
            out.push((start, i - 1));
            start = i + 1;
        }
        i += 1;
    }
    if start < close {
        out.push((start, close - 1));
    }
    out
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn reindent(source: &str, base: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if lines.len() < 2 {
        return source.to_owned();
    }
    let min = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = lines[0].to_owned();
    for l in &lines[1..] {
        out.push('\n');
        if !l.trim().is_empty() {
            out.push_str(base);
            out.push_str(&l[min.min(l.len() - l.trim_start().len())..]);
        }
    }
    out
}

fn ucfirst(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

pub fn class_name(input: &str) -> Result<String, String> {
    let mut name = input.trim().replace('/', "\\");
    if let Some(stripped) = name.strip_suffix(".php") {
        name = stripped.to_owned();
    }
    for prefix in [
        "app\\Http\\Requests\\",
        "App\\Http\\Requests\\",
        "\\App\\Http\\Requests\\",
    ] {
        if let Some(stripped) = name.strip_prefix(prefix) {
            name = stripped.to_owned();
        }
    }
    let name = name.trim_matches('\\');
    if name.is_empty() {
        return Err("Type a class name, such as StoreOrderRequest.".into());
    }
    let valid = |part: &str| {
        let mut c = part.chars();
        c.next()
            .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
            && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    };
    match name.split('\\').find(|part| !valid(part)) {
        Some(part) => Err(format!(
            "'{part}' isn't a valid PHP class name: use letters, digits and _, starting with a letter."
        )),
        None => Ok(name.to_owned()),
    }
}

pub fn file_of(name: &str) -> String {
    format!("app/Http/Requests/{}.php", name.replace('\\', "/"))
}

struct Imports {
    entries: Vec<(String, String)>,
    statements: Vec<(Option<String>, usize, usize)>,
    first_start: Option<usize>,
    last_end: Option<usize>,
}

fn imports(text: &str, toks: &[Tok], class_at: usize) -> Imports {
    let mut entries = Vec::new();
    let mut statements = Vec::new();
    let mut first_start = None;
    let mut last_end = None;
    let mut i = 0;
    while i < class_at {
        if ident(toks.get(i), "use") && toks.get(i + 1).is_some_and(|t| t.kind == Kind::Ident) {
            let Some(end) = (i..class_at).find(|&j| sym(toks.get(j), ";")) else {
                break;
            };
            let stmt = text[toks[i].start..toks[end].end].to_owned();
            let body = stmt
                .trim_start_matches("use")
                .trim()
                .trim_end_matches(';')
                .trim();
            first_start.get_or_insert(toks[i].start);
            let simple = !body.contains('{')
                && !body.starts_with("function ")
                && !body.starts_with("const ");
            let alias_of = |b: &str| {
                b.split_once(" as ")
                    .map(|(_, a)| a.trim().to_owned())
                    .unwrap_or_else(|| b.rsplit('\\').next().unwrap_or(b).to_owned())
            };
            statements.push((simple.then(|| alias_of(body)), toks[i].start, toks[end].end));
            if simple {
                let (path, alias) = match body.split_once(" as ") {
                    Some((p, a)) => (p.trim(), a.trim()),
                    None => (body, body.rsplit('\\').next().unwrap_or(body)),
                };
                entries.push((
                    alias.to_owned(),
                    format!(
                        "use {path}{};",
                        if alias == path.rsplit('\\').next().unwrap_or(path) {
                            String::new()
                        } else {
                            format!(" as {alias}")
                        }
                    ),
                ));
            }
            last_end = Some(toks[end].end);
            i = end + 1;
            continue;
        }
        i += 1;
    }
    Imports {
        entries,
        statements,
        first_start,
        last_end,
    }
}

fn uses_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '\\')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

pub fn extract(
    text: &str,
    cursor: usize,
    chosen: Option<&str>,
    taken: &dyn Fn(&str) -> bool,
) -> Option<Extraction> {
    let toks = tokenize(text);
    let class_at = (0..toks.len()).find(|&i| {
        ident(toks.get(i), "class") && toks.get(i + 1).is_some_and(|t| t.kind == Kind::Ident)
    })?;
    let class = toks[class_at + 1].text.clone();
    let (v, open) = (0..toks.len()).find_map(|i| {
        let t = &toks[i];
        if t.kind != Kind::Ident
            || t.text != "validate"
            || !sym(toks.get(i + 1), "(")
            || !sym(toks.get(i.wrapping_sub(1)), "->")
        {
            return None;
        }
        let close = closing(&toks, i + 1);
        (toks[i.saturating_sub(4)].start <= cursor && cursor <= toks[close].end)
            .then_some((i, i + 1))
    })?;
    let close = closing(&toks, open);
    let receiver = v.checked_sub(2)?;
    let (call_start, request_var, mut arg_list) =
        if toks[receiver].kind == Kind::Var && toks[receiver].text == "this" {
            let a = args(&toks, open, close);
            let first = a.first()?;
            let var = (toks[first.0].kind == Kind::Var && first.0 == first.1)
                .then(|| toks[first.0].text.clone())?;
            (
                receiver,
                Some(var),
                a.into_iter().skip(1).collect::<Vec<_>>(),
            )
        } else if toks[receiver].kind == Kind::Var {
            (
                receiver,
                Some(toks[receiver].text.clone()),
                args(&toks, open, close),
            )
        } else if sym(toks.get(receiver), ")")
            && sym(toks.get(receiver - 1), "(")
            && ident(toks.get(receiver - 2), "request")
        {
            (receiver - 2, None, args(&toks, open, close))
        } else {
            return None;
        };
    if arg_list.is_empty() || !sym(toks.get(arg_list[0].0), "[") {
        return None;
    }
    arg_list.truncate(3);
    let source = |(a, b): (usize, usize)| text[toks[a].start..toks[b].end].to_owned();
    let rules = source(arg_list[0]);
    let messages = arg_list
        .get(1)
        .map(|r| source(*r))
        .filter(|m| m.starts_with('['));
    let attributes = arg_list
        .get(2)
        .map(|r| source(*r))
        .filter(|m| m.starts_with('['));

    let function = (0..call_start).rev().find(|&i| {
        ident(toks.get(i), "function")
            && toks.get(i + 1).is_some_and(|t| t.kind == Kind::Ident)
            && sym(toks.get(i + 2), "(")
    })?;
    let method = toks[function + 1].text.clone();
    let p_open = function + 2;
    let p_close = closing(&toks, p_open);

    let base = class
        .strip_suffix("Controller")
        .filter(|b| !b.is_empty())
        .unwrap_or(&class)
        .to_owned();
    let stem = if method == "__invoke" {
        format!("{base}Request")
    } else {
        format!("{}{base}Request", ucfirst(&method))
    };
    let full = match chosen {
        Some(c) => c.to_owned(),
        None => {
            let mut name = stem.trim_end_matches("Request").to_owned() + "Request";
            let mut n = 2;
            while taken(&file_of(&name)) {
                name = format!("{}{n}Request", stem.trim_end_matches("Request"));
                n += 1;
            }
            name
        }
    };
    let (namespace, name) = match full.rsplit_once('\\') {
        Some((sub, short)) => (format!("App\\Http\\Requests\\{sub}"), short.to_owned()),
        None => ("App\\Http\\Requests".to_owned(), full.clone()),
    };

    let mut edits = Vec::new();
    let typed = (p_open + 1..p_close).find(|&i| {
        toks[i].kind == Kind::Ident
            && toks[i].text.trim_start_matches('\\').rsplit('\\').next() == Some("Request")
            && toks.get(i + 1).is_some_and(|t| {
                t.kind == Kind::Var && request_var.as_ref().is_none_or(|v| *v == t.text)
            })
    });
    let var = match typed {
        Some(i) => {
            edits.push((toks[i].start, toks[i].end, name.clone()));
            toks[i + 1].text.clone()
        }
        None => {
            let v = request_var.clone().unwrap_or_else(|| "request".to_owned());
            if let Some(i) =
                (p_open + 1..p_close).find(|&i| toks[i].kind == Kind::Var && toks[i].text == v)
            {
                edits.push((toks[i].start, toks[i].start, format!("{name} ")));
            } else {
                let at = toks[p_open].end;
                let sep = if p_close == p_open + 1 { "" } else { ", " };
                edits.push((at, at, format!("{name} ${v}{sep}")));
            }
            v
        }
    };

    let stmt_start = (0..call_start)
        .rev()
        .find(|&i| sym(toks.get(i), ";") || sym(toks.get(i), "{") || sym(toks.get(i), "}"))
        .map_or(0, |i| i + 1);
    let bare = stmt_start == call_start && sym(toks.get(close + 1), ";");
    if bare {
        let from = line_start(text, toks[call_start].start);
        let semi = toks[close + 1].end;
        let to = text[semi..].find('\n').map_or(text.len(), |i| semi + i + 1);
        let only = text[from..toks[call_start].start].trim().is_empty()
            && text[semi..to].trim().is_empty();
        if only {
            edits.push((from, to, String::new()));
        } else {
            edits.push((toks[call_start].start, semi, String::new()));
        }
    } else {
        edits.push((
            toks[call_start].start,
            toks[close].end,
            format!("${var}->validated()"),
        ));
    }

    let imported = imports(text, &toks, class_at);
    let fqcn = format!("{namespace}\\{name}");
    match (imported.first_start, imported.last_end) {
        (Some(first), Some(last)) => {
            let mut applied = text.to_owned();
            let mut body: Vec<&(usize, usize, String)> =
                edits.iter().filter(|e| e.0 >= last).collect();
            body.sort_by_key(|e| std::cmp::Reverse(e.0));
            for (s, e, t) in body {
                applied.replace_range(*s..*e, t);
            }
            let outside = |t: &str| format!("{}{}", &t[..first], &t[last..]);
            let (before, after) = (outside(text), outside(&applied));
            let mut kept: Vec<&str> = imported
                .statements
                .iter()
                .filter(|(alias, _, _)| {
                    alias
                        .as_deref()
                        .is_none_or(|a| !uses_word(&before, a) || uses_word(&after, a))
                })
                .map(|(_, s, e)| &text[*s..*e])
                .collect();
            let line = format!("use {fqcn};");
            kept.push(&line);
            edits.push((first, last, kept.join("\n")));
        }
        (_, Some(at)) => edits.push((at, at, format!("\nuse {fqcn};"))),
        _ => {
            let ns = (0..class_at)
                .find(|&i| ident(toks.get(i), "namespace"))
                .and_then(|i| (i..class_at).find(|&j| sym(toks.get(j), ";")));
            let at = match ns {
                Some(j) => toks[j].end,
                None => text.find("<?php").map_or(0, |p| p + 5),
            };
            edits.push((at, at, format!("\n\nuse {fqcn};")));
        }
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));

    let used = format!(
        "{rules}{}{}",
        messages.as_deref().unwrap_or(""),
        attributes.as_deref().unwrap_or("")
    );
    let mut uses: Vec<String> = imported
        .entries
        .iter()
        .filter(|(alias, _)| uses_word(&used, alias))
        .map(|(_, line)| line.clone())
        .collect();
    uses.push("use Illuminate\\Foundation\\Http\\FormRequest;".to_owned());
    uses.sort();
    uses.dedup();
    let mut content = format!(
        "<?php\n\nnamespace {namespace};\n\n{}\n\nclass {name} extends FormRequest\n{{\n    public function authorize(): bool\n    {{\n        return true;\n    }}\n\n    public function rules(): array\n    {{\n        return {};\n    }}\n",
        uses.join("\n"),
        reindent(&rules, "        ")
    );
    for (method, array) in [("messages", messages), ("attributes", attributes)] {
        if let Some(array) = array {
            content.push_str(&format!(
                "\n    public function {method}(): array\n    {{\n        return {};\n    }}\n",
                reindent(&array, "        ")
            ));
        }
    }
    content.push_str("}\n");
    Some(Extraction {
        file: file_of(&full),
        class: full,
        content,
        edits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTROLLER: &str = "<?php\n\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Order;\nuse Illuminate\\Http\\Request;\nuse Illuminate\\Validation\\Rule;\n\nclass OrderController extends Controller\n{\n    public function store(Request $request)\n    {\n        $data = $request->validate([\n            'status' => ['required', Rule::in(['paid', 'open'])],\n            'total' => 'required|integer',\n        ], [\n            'status.in' => 'Unknown status.',\n        ]);\n\n        return Order::create($data);\n    }\n}\n";

    fn apply(text: &str, edits: &[(usize, usize, String)]) -> String {
        let mut out = text.to_owned();
        for (s, e, t) in edits {
            out.replace_range(*s..*e, t);
        }
        out
    }

    #[test]
    fn extracts_rules_messages_and_imports_into_a_form_request() {
        let at = CONTROLLER.find("validate").unwrap();
        let x = extract(CONTROLLER, at, None, &|_| false).unwrap();
        assert_eq!(x.class, "StoreOrderRequest");
        assert_eq!(x.file, "app/Http/Requests/StoreOrderRequest.php");
        assert_eq!(
            x.content,
            "<?php\n\nnamespace App\\Http\\Requests;\n\nuse Illuminate\\Foundation\\Http\\FormRequest;\nuse Illuminate\\Validation\\Rule;\n\nclass StoreOrderRequest extends FormRequest\n{\n    public function authorize(): bool\n    {\n        return true;\n    }\n\n    public function rules(): array\n    {\n        return [\n            'status' => ['required', Rule::in(['paid', 'open'])],\n            'total' => 'required|integer',\n        ];\n    }\n\n    public function messages(): array\n    {\n        return [\n            'status.in' => 'Unknown status.',\n        ];\n    }\n}\n"
        );
        let after = apply(CONTROLLER, &x.edits);
        assert!(after.contains("namespace App\\Http\\Controllers;\n\nuse App\\Models\\Order;\nuse App\\Http\\Requests\\StoreOrderRequest;\n\nclass"), "{after}");
        assert!(
            after.contains("public function store(StoreOrderRequest $request)"),
            "{after}"
        );
        assert!(
            after.contains(
                "        $data = $request->validated();\n\n        return Order::create($data);"
            ),
            "{after}"
        );
    }

    #[test]
    fn a_bare_validate_call_is_removed_and_the_helper_gets_a_parameter() {
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nclass PostController\n{\n    public function update(Post $post)\n    {\n        request()->validate(['title' => 'required']);\n        $post->update(request()->all());\n    }\n}\n";
        let x = extract(text, text.find("validate").unwrap(), None, &|f| {
            f.ends_with("UpdatePostRequest.php")
        })
        .unwrap();
        assert_eq!(x.class, "UpdatePost2Request");
        let after = apply(text, &x.edits);
        assert!(
            after.contains("public function update(UpdatePost2Request $request, Post $post)"),
            "{after}"
        );
        assert!(!after.contains("validate("), "{after}");
        assert!(
            after.contains(
                "namespace App\\Http\\Controllers;\n\nuse App\\Http\\Requests\\UpdatePost2Request;"
            ),
            "{after}"
        );
        assert!(
            x.content
                .contains("        return ['title' => 'required'];"),
            "{}",
            x.content
        );
    }

    #[test]
    fn this_validate_with_the_request_argument() {
        let text = "<?php\nclass UserController\n{\n    public function __invoke(\\Illuminate\\Http\\Request $req)\n    {\n        $this->validate($req, ['email' => 'email']);\n    }\n}\n";
        let x = extract(text, text.find("validate").unwrap(), None, &|_| false).unwrap();
        assert_eq!(x.class, "UserRequest");
        let after = apply(text, &x.edits);
        assert!(after.contains("__invoke(UserRequest $req)"), "{after}");
        assert!(extract(text, 0, None, &|_| false).is_none());
    }

    #[test]
    fn a_chosen_name_sets_the_class_the_folder_and_the_namespace() {
        let at = CONTROLLER.find("validate").unwrap();
        let x = extract(CONTROLLER, at, Some(r"Admin\CreateOrder"), &|_| true).unwrap();
        assert_eq!(x.class, r"Admin\CreateOrder");
        assert_eq!(x.file, "app/Http/Requests/Admin/CreateOrder.php");
        assert!(
            x.content.contains(r"namespace App\Http\Requests\Admin;"),
            "{}",
            x.content
        );
        assert!(
            x.content.contains("class CreateOrder extends FormRequest"),
            "{}",
            x.content
        );
        let after = apply(CONTROLLER, &x.edits);
        assert!(
            after.contains(r"use App\Http\Requests\Admin\CreateOrder;"),
            "{after}"
        );
        assert!(
            after.contains("public function store(CreateOrder $request)"),
            "{after}"
        );
    }

    #[test]
    fn typed_names_are_normalised_or_refused() {
        assert_eq!(
            class_name("  StoreOrderRequest ").unwrap(),
            "StoreOrderRequest"
        );
        assert_eq!(
            class_name("Admin/StoreOrderRequest.php").unwrap(),
            r"Admin\StoreOrderRequest"
        );
        assert_eq!(
            class_name("app/Http/Requests/Admin/Store.php").unwrap(),
            r"Admin\Store"
        );
        assert_eq!(class_name(r"App\Http\Requests\Store").unwrap(), "Store");
        assert!(class_name("").is_err());
        assert!(class_name("1Store").is_err());
        assert!(class_name("Store-Order").is_err());
        assert!(class_name("Admin//Store").is_err());
        assert_eq!(file_of(r"Admin\Store"), "app/Http/Requests/Admin/Store.php");
    }
}
