use crate::php::{Kind, tokenize};
use crate::resources::{Scan, Symbol, role};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub name: String,
    pub rules: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub param: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteInfo {
    pub methods: Vec<String>,
    pub uri: String,
    pub name: String,
    pub action: String,
    pub form_request: Option<String>,
    pub fields: Vec<Field>,
    pub bindings: Vec<Binding>,
    pub api: bool,
}

pub fn uri_params(uri: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = uri;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let inner = &rest[open + 1..open + close];
        let name = inner
            .trim_end_matches('?')
            .split(':')
            .next()
            .unwrap_or_default();
        if !name.is_empty() {
            out.push(name.to_owned());
        }
        rest = &rest[open + close + 1..];
    }
    out
}

fn params_of(signature: &str) -> Vec<(String, String)> {
    signature
        .split(',')
        .filter_map(|p| {
            let p = p.split('=').next().unwrap_or_default().trim();
            let mut words: Vec<&str> = p.split_whitespace().collect();
            let var = words.pop()?.strip_prefix('$')?;
            let ty = words.last().copied().unwrap_or_default();
            let ty = ty.trim_start_matches('?').rsplit('\\').next().unwrap_or(ty);
            Some((ty.to_owned(), var.trim_start_matches('&').to_owned()))
        })
        .collect()
}

fn clean_key(key: &str) -> Option<String> {
    let k = key.trim().trim_matches(['\'', '"']);
    (!k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || "_.*-".contains(c)))
        .then(|| k.to_owned())
}

fn rules_text(value: &str) -> String {
    let value = value.trim();
    let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return value.trim_matches(['\'', '"']).to_owned();
    };
    tokenize(inner, 0, inner.len())
        .iter()
        .filter_map(|t| match &t.kind {
            Kind::Str { value, .. } if !value.is_empty() => Some(value.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn pairs(body: &str) -> Vec<(String, String)> {
    let tokens = tokenize(body, 0, body.len());
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != Kind::DoubleArrow || i == 0 {
            continue;
        }
        let Kind::Str { value: key, .. } = &tokens[i - 1].kind else {
            continue;
        };
        let mut depth = 0i32;
        let mut end = tokens.len();
        for (k, v) in tokens.iter().enumerate().skip(i + 1) {
            match v.kind {
                Kind::Open(_) => depth += 1,
                Kind::Close(_) if depth == 0 => {
                    end = k;
                    break;
                }
                Kind::Close(_) => depth -= 1,
                Kind::Comma if depth == 0 => {
                    end = k;
                    break;
                }
                _ => {}
            }
        }
        if end <= i + 1 {
            continue;
        }
        let value = body[tokens[i + 1].start..tokens[end - 1].end].to_owned();
        out.push((key.clone(), value));
    }
    out
}

fn fields_from(body: &str) -> Vec<Field> {
    let mut out: Vec<Field> = Vec::new();
    for (key, value) in pairs(body) {
        let Some(name) = clean_key(&key) else {
            continue;
        };
        if out.iter().any(|f| f.name == name) {
            continue;
        }
        let rules = rules_text(&value);
        let required = rules
            .split('|')
            .any(|r| r == "required" || r.starts_with("required_"))
            && !rules.split('|').any(|r| r == "sometimes");
        out.push(Field {
            name,
            rules,
            required,
        });
    }
    out
}

fn inline_validation(body: &str) -> Option<&str> {
    let at = body.find("validate(")?;
    let rest = &body[at..];
    let open = rest.find('[')?;
    let mut depth = 0i32;
    for (i, c) in rest[open..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[open..open + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_class<'a>(scan: &'a Scan, fqcn: &str) -> Option<&'a Symbol> {
    let fqcn = fqcn.trim_start_matches('\\');
    let short = fqcn.rsplit('\\').next().unwrap_or(fqcn);
    scan.symbols
        .iter()
        .filter(|s| s.keyword == "class" && s.name == short)
        .find(|s| format!("{}\\{}", s.namespace, s.name).trim_start_matches('\\') == fqcn)
        .or_else(|| {
            scan.symbols
                .iter()
                .find(|s| s.keyword == "class" && s.name == short)
        })
}

pub fn info(scan: &Scan, method: &str, uri: &str, name: &str, action: &str) -> RouteInfo {
    let resource = method
        .eq_ignore_ascii_case("resource")
        .then(|| {
            let function = action
                .rsplit_once("::")
                .or_else(|| action.rsplit_once('@'))
                .map_or("", |(_, f)| f);
            resource_route(uri, function)
        })
        .flatten();
    let (method, uri) = match &resource {
        Some((verb, path)) => (verb.as_str(), path.as_str()),
        None => (method, uri),
    };
    let methods: Vec<String> = method
        .split('|')
        .map(|m| m.trim().to_ascii_uppercase())
        .filter(|m| !m.is_empty() && m != "HEAD")
        .collect();
    let mut info = RouteInfo {
        methods: if methods.is_empty() {
            vec!["GET".into()]
        } else {
            methods
        },
        uri: uri.to_owned(),
        name: name.to_owned(),
        action: action.to_owned(),
        form_request: None,
        fields: Vec::new(),
        bindings: uri_params(uri)
            .into_iter()
            .map(|param| Binding { param, model: None })
            .collect(),
        api: uri.trim_start_matches('/').starts_with("api/")
            || uri.trim_start_matches('/') == "api",
    };
    let (class, method_name) = action
        .split_once("::")
        .or_else(|| action.split_once('@'))
        .unwrap_or((action, "__invoke"));
    let Some(controller) = find_class(scan, class) else {
        return info;
    };
    let Some(m) = controller.methods.iter().find(|m| m.name == method_name) else {
        return info;
    };
    for (ty, var) in params_of(&m.params) {
        let typed = scan
            .symbols
            .iter()
            .find(|s| s.keyword == "class" && s.name == ty);
        if let Some(s) = typed {
            match role(s) {
                "Form request" if info.form_request.is_none() => {
                    info.form_request = Some(s.name.clone());
                    if let Some(rules) = s.methods.iter().find(|m| m.name == "rules") {
                        info.fields = fields_from(&rules.body);
                    }
                }
                "Model" => {
                    if let Some(b) = info.bindings.iter_mut().find(|b| b.param == var) {
                        b.model = Some(s.name.clone());
                    }
                }
                _ => {}
            }
        }
    }
    if info.fields.is_empty()
        && let Some(array) = inline_validation(&m.body)
    {
        info.fields = fields_from(array);
    }
    info
}

pub fn sample(field: &Field) -> Value {
    let rules: Vec<&str> = field.rules.split('|').collect();
    let has = |r: &str| {
        rules
            .iter()
            .any(|x| *x == r || x.starts_with(&format!("{r}:")))
    };
    let name = field.name.to_ascii_lowercase();
    if has("boolean") || has("accepted") {
        json!(true)
    } else if has("integer") || has("numeric") || name.ends_with("_id") || name == "id" {
        json!(1)
    } else if has("array") {
        json!([])
    } else if has("email") || name.contains("email") {
        json!("user@example.com")
    } else if has("date") {
        json!("2026-01-01")
    } else if has("url") {
        json!("https://example.com")
    } else if name.contains("password") {
        json!("password")
    } else if field.required {
        json!("Example")
    } else {
        json!("")
    }
}

fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("ies") {
        format!("{stem}y")
    } else if word.ends_with("sses")
        || word.ends_with("shes")
        || word.ends_with("ches")
        || word.ends_with("xes")
    {
        word[..word.len() - 2].to_owned()
    } else {
        word.strip_suffix('s').unwrap_or(word).to_owned()
    }
}

fn exists_target(rules: &str) -> Option<(String, String)> {
    let rule = rules.split('|').find_map(|r| r.strip_prefix("exists:"))?;
    let mut parts = rule.split(',');
    let table = parts.next()?.rsplit('.').next()?.trim();
    let column = parts.next().map_or("id", str::trim);
    (!table.is_empty()).then(|| (table.to_owned(), column.to_owned()))
}

pub fn sample_body(info: &RouteInfo) -> Value {
    let mut map = serde_json::Map::new();
    for f in info.fields.iter().filter(|f| !f.name.contains('*')) {
        map.insert(f.name.clone(), sample(f));
    }
    Value::Object(map)
}

fn studly(text: &str) -> String {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_ascii_uppercase().to_string() + c.as_str()
            })
        })
        .collect()
}

pub struct TestOptions<'a> {
    pub pest: bool,
    pub auth: bool,
    pub user_model: Option<&'a str>,
    pub has_factory: &'a dyn Fn(&str) -> bool,
}

fn php_value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(_) => "[]".into(),
        Value::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        _ => "null".into(),
    }
}

fn test_base(action: &str, method: &str, uri: &str) -> String {
    let (class, function) = action
        .split_once("::")
        .or_else(|| action.split_once('@'))
        .unwrap_or(("", ""));
    let controller = class.rsplit('\\').next().unwrap_or_default();
    let base = if controller.is_empty() {
        studly(&format!("{} {}", method.to_lowercase(), uri))
    } else {
        format!(
            "{}{}",
            controller.trim_end_matches("Controller"),
            studly(function)
        )
    };
    if base.is_empty() {
        "Route".to_owned()
    } else {
        base
    }
}

pub fn resource_route(base: &str, action: &str) -> Option<(String, String)> {
    let base = base.trim_end_matches('/');
    let last = base.rsplit('/').next().unwrap_or(base);
    let param = format!("{{{}}}", singular(last).replace('-', "_"));
    let (verb, uri) = match action {
        "index" => ("GET", base.to_owned()),
        "create" => ("GET", format!("{base}/create")),
        "store" => ("POST", base.to_owned()),
        "show" => ("GET", format!("{base}/{param}")),
        "edit" => ("GET", format!("{base}/{param}/edit")),
        "update" => ("PUT", format!("{base}/{param}")),
        "destroy" => ("DELETE", format!("{base}/{param}")),
        _ => return None,
    };
    Some((verb.to_owned(), uri))
}

pub fn test_path(action: &str, method: &str, uri: &str) -> String {
    format!("tests/Feature/{}Test.php", test_base(action, method, uri))
}

pub fn test_file(info: &RouteInfo, opts: &TestOptions) -> (String, String) {
    let base = test_base(&info.action, &info.methods[0], &info.uri);
    let verb = info.methods[0].to_lowercase();
    let json = info.api;
    let call = if json {
        format!("{verb}Json")
    } else {
        verb.clone()
    };
    let mut setup = Vec::new();
    let mut imports = vec!["use Illuminate\\Foundation\\Testing\\RefreshDatabase;".to_owned()];
    let mut route_args = Vec::new();
    for b in &info.bindings {
        match &b.model {
            Some(model) if (opts.has_factory)(model) => {
                setup.push(format!("${} = {model}::factory()->create();", b.param));
                imports.push(format!("use App\\Models\\{model};"));
                route_args.push(format!("'{}' => ${}", b.param, b.param));
            }
            _ => route_args.push(format!("'{}' => 1", b.param)),
        }
    }
    let acting = match opts.user_model {
        Some(user) if opts.auth && (opts.has_factory)(user) => {
            if !imports.iter().any(|i| i.ends_with(&format!("\\{user};"))) {
                imports.push(format!("use App\\Models\\{user};"));
            }
            setup.push(format!("$actor = {user}::factory()->create();"));
            true
        }
        _ => false,
    };
    let url = if info.name.is_empty() {
        let mut uri = format!("/{}", info.uri.trim_start_matches('/'));
        for b in &info.bindings {
            let value = if b.model.as_deref().is_some_and(|m| (opts.has_factory)(m)) {
                format!("{{${}->getRouteKey()}}", b.param)
            } else {
                "1".into()
            };
            for pattern in [format!("{{{}}}", b.param), format!("{{{}?}}", b.param)] {
                uri = uri.replace(&pattern, &value);
            }
        }
        format!("\"{uri}\"")
    } else if route_args.is_empty() {
        format!("route('{}')", info.name)
    } else {
        format!("route('{}', [{}])", info.name, route_args.join(", "))
    };
    let sends_body = info.methods[0] != "GET" && info.methods[0] != "DELETE";
    let mut payload: Vec<String> = Vec::new();
    for f in info
        .fields
        .iter()
        .filter(|f| !f.name.contains('*') && !f.name.contains('.'))
    {
        let related = exists_target(&f.rules).and_then(|(table, column)| {
            let model = studly(&singular(&table));
            (opts.has_factory)(&model).then_some((model, column))
        });
        match related {
            Some((model, column)) => {
                let var = {
                    let mut c = model.chars();
                    c.next().map_or_else(String::new, |first| {
                        first.to_ascii_lowercase().to_string() + c.as_str()
                    })
                };
                let line = format!("${var} = {model}::factory()->create();");
                if !setup.contains(&line) {
                    setup.push(line);
                    imports.push(format!("use App\\Models\\{model};"));
                }
                payload.push(format!("'{}' => ${var}->{column}", f.name));
            }
            None => payload.push(format!("'{}' => {}", f.name, php_value(&sample(f)))),
        }
    }
    let required: Vec<&Field> = info
        .fields
        .iter()
        .filter(|f| f.required && !f.name.contains('*') && !f.name.contains('.'))
        .collect();
    let subject = "$this";
    let request = |data: &str| {
        let actor = if acting { "->actingAs($actor)" } else { "" };
        if sends_body {
            format!("{subject}{actor}->{call}({url}, {data})")
        } else {
            format!("{subject}{actor}->{call}({url})")
        }
    };
    let happy_assert = if sends_body {
        if json {
            "->assertJsonMissingValidationErrors()"
        } else {
            "->assertSessionHasNoErrors()"
        }
    } else {
        "->assertSuccessful()"
    };
    let error_assert = |field: &str| {
        if json {
            format!("->assertJsonValidationErrors('{field}')")
        } else {
            format!("->assertSessionHasErrors('{field}')")
        }
    };
    imports.sort();
    imports.dedup();
    let payload_php = format!("[{}]", payload.join(", "));
    let label = format!("{} /{}", info.methods[0], info.uri.trim_start_matches('/'));
    let mut body = String::new();
    if opts.pest {
        let path = format!("tests/Feature/{base}Test.php");
        body.push_str("<?php\n\n");
        for i in &imports {
            body.push_str(i);
            body.push('\n');
        }
        body.push_str("\nuses(RefreshDatabase::class);\n\n");
        body.push_str(&format!("it('handles {label}', function () {{\n"));
        for s in &setup {
            body.push_str(&format!("    {s}\n"));
        }
        let gap = if setup.is_empty() { "" } else { "\n" };
        body.push_str(&format!(
            "{gap}    {}\n        {happy_assert};\n}});\n",
            request(&payload_php)
        ));
        if sends_body && !required.is_empty() {
            let names: Vec<String> = required.iter().map(|f| format!("'{}'", f.name)).collect();
            body.push_str(&format!(
                "\nit('requires each field of {label}', function (string $field) {{\n"
            ));
            for s in &setup {
                body.push_str(&format!("    {s}\n"));
            }
            body.push_str(&format!("    $payload = {payload_php};\n    unset($payload[$field]);\n\n    {}\n        {};\n}})->with([{}]);\n", request("$payload"), error_assert_dyn(json), names.join(", ")));
        }
        (path, body)
    } else {
        let path = format!("tests/Feature/{base}Test.php");
        body.push_str("<?php\n\nnamespace Tests\\Feature;\n\n");
        for i in &imports {
            body.push_str(i);
            body.push('\n');
        }
        body.push_str("use Tests\\TestCase;\n\n");
        body.push_str(&format!(
            "class {base}Test extends TestCase\n{{\n    use RefreshDatabase;\n\n"
        ));
        body.push_str("    public function test_it_handles_the_request(): void\n    {\n");
        for s in &setup {
            body.push_str(&format!("        {s}\n"));
        }
        let gap = if setup.is_empty() { "" } else { "\n" };
        body.push_str(&format!(
            "{gap}        {}\n            {happy_assert};\n    }}\n",
            request(&payload_php)
        ));
        if sends_body {
            for f in &required {
                let method_name = f.name.replace(['-', '.'], "_");
                body.push_str(&format!(
                    "\n    public function test_it_requires_{method_name}(): void\n    {{\n"
                ));
                for s in &setup {
                    body.push_str(&format!("        {s}\n"));
                }
                body.push_str(&format!("        $payload = {payload_php};\n        unset($payload['{}']);\n\n        {}\n            {};\n    }}\n", f.name, request("$payload"), error_assert(&f.name)));
            }
        }
        body.push_str("}\n");
        (path, body)
    }
}

fn error_assert_dyn(json: bool) -> &'static str {
    if json {
        "->assertJsonValidationErrors($field)"
    } else {
        "->assertSessionHasErrors($field)"
    }
}

pub fn middleware_of(stdout: &str, method: &str, uri: &str) -> Option<Vec<String>> {
    let start = stdout.find('[')?;
    let end = stdout.rfind(']')? + 1;
    let routes: Vec<Value> = serde_json::from_str(&stdout[start..end]).ok()?;
    let uri = uri.trim_start_matches('/');
    let route = routes.iter().find(|r| {
        r["uri"].as_str().map(|u| u.trim_start_matches('/')) == Some(uri)
            && r["method"]
                .as_str()
                .is_some_and(|m| m.split('|').any(|x| x.eq_ignore_ascii_case(method)))
    })?;
    Some(match &route["middleware"] {
        Value::Array(items) => items
            .iter()
            .filter_map(|m| m.as_str().map(str::to_owned))
            .collect(),
        Value::String(s) => s.split([',', '\n']).map(|x| x.trim().to_owned()).collect(),
        _ => Vec::new(),
    })
}

pub fn needs_auth(middleware: &[String]) -> bool {
    middleware.iter().any(|m| {
        let short = m.rsplit('\\').next().unwrap_or(m);
        m.starts_with("auth")
            || m.starts_with("verified")
            || m.starts_with("can:")
            || short.starts_with("Authenticate")
            || short.starts_with("Authorize")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::scan_text;

    fn scan() -> Scan {
        let mut scan = Scan::default();
        for (path, text) in [
            (
                "app/Http/Controllers/OrderController.php",
                "<?php\nnamespace App\\Http\\Controllers;\n\nuse App\\Http\\Requests\\StoreOrderRequest;\nuse App\\Models\\User;\n\nclass OrderController extends Controller\n{\n    public function store(StoreOrderRequest $request, User $user)\n    {\n        return $user->orders()->create($request->validated());\n    }\n\n    public function rate(Request $request)\n    {\n        $request->validate(['stars' => 'required|integer|between:1,5', 'note' => ['nullable', 'string']]);\n    }\n}\n",
            ),
            (
                "app/Http/Requests/StoreOrderRequest.php",
                "<?php\nnamespace App\\Http\\Requests;\n\nclass StoreOrderRequest extends FormRequest\n{\n    public function rules(): array\n    {\n        return [\n            'total' => ['required', 'numeric'],\n            'email' => 'required|email',\n            'notes' => 'sometimes|required|string',\n            'paid' => 'boolean',\n            'items.*.sku' => 'required',\n        ];\n    }\n}\n",
            ),
            (
                "app/Models/User.php",
                "<?php\nnamespace App\\Models;\n\nclass User extends Authenticatable\n{\n}\n",
            ),
        ] {
            let part = scan_text(path, text);
            scan.symbols.extend(part.symbols);
        }
        scan
    }

    #[test]
    fn reads_fields_and_bindings_of_a_route() {
        let info = info(
            &scan(),
            "POST",
            "users/{user}/orders",
            "orders.store",
            "App\\Http\\Controllers\\OrderController::store",
        );
        assert_eq!(info.form_request.as_deref(), Some("StoreOrderRequest"));
        let fields: Vec<(String, bool)> = info
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.required))
            .collect();
        assert_eq!(
            fields,
            [
                ("total".to_owned(), true),
                ("email".to_owned(), true),
                ("notes".to_owned(), false),
                ("paid".to_owned(), false),
                ("items.*.sku".to_owned(), true),
            ]
        );
        assert_eq!(
            info.bindings,
            [Binding {
                param: "user".into(),
                model: Some("User".into())
            }]
        );
        assert_eq!(
            sample_body(&info),
            json!({ "total": 1, "email": "user@example.com", "notes": "", "paid": true })
        );
        let inline = info_inline();
        assert_eq!(
            inline
                .fields
                .iter()
                .map(|f| f.rules.as_str())
                .collect::<Vec<_>>(),
            ["required|integer|between:1,5", "nullable|string"]
        );
        assert_eq!(
            uri_params("posts/{post}/comments/{comment?}/{slug:slug}"),
            ["post", "comment", "slug"]
        );
    }

    fn info_inline() -> RouteInfo {
        info(
            &scan(),
            "PUT|PATCH",
            "api/orders/rate",
            "",
            "App\\Http\\Controllers\\OrderController@rate",
        )
    }

    #[test]
    fn writes_pest_and_phpunit_tests_for_a_route() {
        let info = info(
            &scan(),
            "POST",
            "users/{user}/orders",
            "orders.store",
            "App\\Http\\Controllers\\OrderController::store",
        );
        let factory = |m: &str| m == "User";
        let (path, pest) = test_file(
            &info,
            &TestOptions {
                pest: true,
                auth: true,
                user_model: Some("User"),
                has_factory: &factory,
            },
        );
        assert_eq!(path, "tests/Feature/OrderStoreTest.php");
        assert!(pest.contains("uses(RefreshDatabase::class);"));
        assert!(pest.contains("$user = User::factory()->create();"));
        assert!(pest.contains("$this->actingAs($actor)->post(route('orders.store', ['user' => $user]), ['total' => 1, 'email' => 'user@example.com', 'notes' => '', 'paid' => true])"));
        assert!(pest.contains("->assertSessionHasNoErrors();"));
        assert!(pest.contains("->with(['total', 'email']);"));
        assert!(pest.contains("->assertSessionHasErrors($field);"));
        let (_, unit) = test_file(
            &info,
            &TestOptions {
                pest: false,
                auth: false,
                user_model: Some("User"),
                has_factory: &factory,
            },
        );
        assert!(unit.contains("class OrderStoreTest extends TestCase"));
        assert!(unit.contains("public function test_it_requires_total(): void"));
        assert!(unit.contains("unset($payload['email']);"));
        assert!(!unit.contains("actingAs"));
        let (path, api) = test_file(
            &info_inline(),
            &TestOptions {
                pest: true,
                auth: false,
                user_model: None,
                has_factory: &factory,
            },
        );
        assert_eq!(path, "tests/Feature/OrderRateTest.php");
        assert!(api.contains("$this->putJson(\"/api/orders/rate\", ['stars' => 1, 'note' => ''])"));
        assert!(api.contains("->assertJsonValidationErrors($field);"));
    }

    #[test]
    fn creates_related_rows_for_exists_rules() {
        let info = RouteInfo {
            methods: vec!["POST".into()],
            uri: "products".into(),
            name: "products.store".into(),
            action: "App\\Http\\Controllers\\ProductController::store".into(),
            form_request: None,
            fields: vec![
                Field {
                    name: "category_id".into(),
                    rules: "required|exists:categories,id".into(),
                    required: true,
                },
                Field {
                    name: "name".into(),
                    rules: "required|string|max:120".into(),
                    required: true,
                },
            ],
            bindings: Vec::new(),
            api: false,
        };
        let factory = |m: &str| m == "Category" || m == "User";
        let (_, pest) = test_file(
            &info,
            &TestOptions {
                pest: true,
                auth: true,
                user_model: Some("User"),
                has_factory: &factory,
            },
        );
        assert!(pest.contains("use App\\Models\\Category;"));
        assert!(pest.contains("$category = Category::factory()->create();"));
        assert!(pest.contains("['category_id' => $category->id, 'name' => 'Example']"));
        assert!(needs_auth(&["web".into(), "can:manage-products".into()]));
        assert_eq!(singular("categories"), "category");
        assert_eq!(singular("addresses"), "address");
        assert_eq!(singular("orders"), "order");
    }

    #[test]
    fn reads_the_middleware_of_a_route() {
        let out = "[{\"method\":\"POST\",\"uri\":\"users/{user}/orders\",\"middleware\":[\"web\",\"auth\"]},{\"method\":\"GET|HEAD\",\"uri\":\"/\",\"middleware\":[\"web\"]}]";
        let m = middleware_of(out, "POST", "/users/{user}/orders").unwrap();
        assert!(needs_auth(&m));
        assert!(!needs_auth(&middleware_of(out, "GET", "/").unwrap()));
        assert!(middleware_of("nope", "GET", "/").is_none());
    }

    #[test]
    fn resource_routes_and_test_paths() {
        assert_eq!(
            resource_route("/products", "store"),
            Some(("POST".into(), "/products".into()))
        );
        assert_eq!(
            resource_route("/order-items", "update"),
            Some(("PUT".into(), "/order-items/{order_item}".into()))
        );
        assert_eq!(
            resource_route("/categories", "edit"),
            Some(("GET".into(), "/categories/{category}/edit".into()))
        );
        assert_eq!(resource_route("/products", "export"), None);
        let resource = info(
            &scan(),
            "RESOURCE",
            "/orders",
            "orders.store",
            "App\\Http\\Controllers\\OrderController::store",
        );
        assert_eq!(resource.methods, ["POST"]);
        assert_eq!(resource.uri, "/orders");
        assert_eq!(resource.form_request.as_deref(), Some("StoreOrderRequest"));
        let shown = info(
            &scan(),
            "resource",
            "/orders",
            "orders.show",
            "OrderController@show",
        );
        assert_eq!(
            (shown.methods, shown.uri),
            (vec!["GET".to_owned()], "/orders/{order}".to_owned())
        );
        assert_eq!(shown.bindings[0].param, "order");
        assert_eq!(
            test_path(
                "App\\Http\\Controllers\\ProductController::store",
                "POST",
                "/products"
            ),
            "tests/Feature/ProductStoreTest.php"
        );
        assert_eq!(
            test_path("ProductController@index", "GET", "/products"),
            "tests/Feature/ProductIndexTest.php"
        );
        assert_eq!(
            test_path("", "GET", "/health"),
            "tests/Feature/GetHealthTest.php"
        );
    }
}
