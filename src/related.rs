use crate::index::{RefKind, is_model, parse_class, parse_routes, relation_of};
use crate::indexer::{imports, resolve};
use crate::php::{self, Kind, Token, tokenize};
use crate::refs::{self, Want};
use crate::resources;
use std::collections::{BTreeMap, BTreeSet};

pub const KEY: &str = "laravel-related#";

pub fn model_key(model: &str) -> String {
    format!("{KEY}model:{model}")
}

pub fn view_key(view: &str) -> String {
    format!("{KEY}view:{view}")
}

pub fn view_file_key(view: &str) -> String {
    format!("{KEY}view-file:{view}")
}

pub fn action_key(controller: &str, method: &str) -> String {
    format!("{KEY}action:{}@{method}", short(controller))
}

pub fn event_key(fqn: &str) -> String {
    format!("{KEY}event:{}", fqn.trim_start_matches('\\'))
}

fn short(name: &str) -> &str {
    name.trim_start_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or(name)
}

#[derive(Clone, Debug, Default)]
pub struct RouteRef {
    pub name: String,
    pub uri: String,
    pub action: String,
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    pub models: BTreeMap<String, String>,
    pub routes: Vec<RouteRef>,
    pub event_models: BTreeMap<String, Vec<String>>,
    pub cast_models: BTreeMap<String, Vec<String>>,
}

impl Context {
    fn model_of_table(&self, table: &str) -> Option<&str> {
        self.models
            .iter()
            .find(|(_, t)| t.as_str() == table)
            .map(|(m, _)| m.as_str())
    }

    fn is_model(&self, name: &str) -> bool {
        self.models.contains_key(name)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub markers: Vec<(String, usize, usize)>,
    pub definitions: Vec<(String, usize, usize)>,
}

impl Found {
    fn member(&mut self, key: String, start: usize, end: usize) {
        if !self
            .markers
            .iter()
            .any(|(k, s, _)| *k == key && *s == start)
        {
            self.markers.push((key.clone(), start, end));
            self.definitions.push((key, start, end));
        }
    }
}

pub fn view_name(rel_path: &str) -> Option<String> {
    let name = rel_path
        .strip_prefix("resources/views/")?
        .strip_suffix(".blade.php")?;
    Some(name.replace('/', "."))
}

fn kebab(word: &str) -> String {
    let mut out = String::new();
    for (i, c) in word.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn livewire_view(rel_path: &str) -> Option<String> {
    let rest = rel_path
        .strip_prefix("app/Livewire/")
        .or_else(|| rel_path.strip_prefix("app/Http/Livewire/"))?
        .strip_suffix(".php")?;
    let parts: Vec<String> = rest.split('/').map(kebab).collect();
    Some(format!("livewire.{}", parts.join(".")))
}

fn view_refs(text: &str) -> Vec<(String, usize, usize)> {
    let scope = crate::scope_of(text);
    let mut out = Vec::new();
    for arg in php::args_in(text, false) {
        if !arg.closed || arg.concat {
            continue;
        }
        if matches!(arg.param.as_deref(), Some("view" | "markdown" | "text"))
            && !arg.value.is_empty()
            && !arg.value.contains(' ')
        {
            out.push((arg.value.clone(), arg.start, arg.start + arg.value.len()));
            continue;
        }
        for seg in refs::segments(&arg, &scope) {
            if seg.want != Want::Kind(RefKind::View) {
                continue;
            }
            let Some(value) = arg.value.get(seg.start..seg.end) else {
                continue;
            };
            if !value.is_empty() {
                let start = arg.start + seg.start;
                out.push((value.to_owned(), start, start + value.len()));
            }
        }
    }
    out
}

fn ident(t: Option<&Token>) -> Option<&str> {
    match &t?.kind {
        Kind::Ident(s) => Some(s),
        _ => None,
    }
}

fn schema_tables(text: &str) -> Vec<(String, usize, usize)> {
    let toks = tokenize(text, 0, text.len());
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if ident(toks.get(i)).is_some_and(|s| short(s) == "Schema")
            && toks.get(i + 1).is_some_and(|t| t.kind == Kind::DoubleColon)
            && matches!(ident(toks.get(i + 2)), Some("create" | "table"))
            && toks.get(i + 3).is_some_and(|t| t.kind == Kind::Open('('))
            && let Some(Token {
                kind: Kind::Str { value, .. },
                start,
                ..
            }) = toks.get(i + 4)
        {
            let quoted = matches!(text.as_bytes().get(*start), Some(b'\'' | b'"'));
            let at = start + usize::from(quoted);
            out.push((value.clone(), at, at + value.len()));
        }
    }
    out
}

fn strings(text: &str) -> Vec<(String, usize, usize)> {
    tokenize(text, 0, text.len())
        .into_iter()
        .filter_map(|t| {
            let Kind::Str { value, .. } = t.kind else {
                return None;
            };
            let quoted = matches!(text.as_bytes().get(t.start), Some(b'\'' | b'"'));
            let at = t.start + usize::from(quoted);
            let end = at + value.len();
            Some((value, at, end))
        })
        .collect()
}

fn uri_matches(pattern: &str, literal: &str) -> bool {
    let literal = literal.split('?').next().unwrap_or_default();
    let p: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let l: Vec<&str> = literal.trim_matches('/').split('/').collect();
    p.len() == l.len()
        && p.iter()
            .zip(&l)
            .all(|(a, b)| (a.starts_with('{') && !b.is_empty()) || a == b)
}

fn first_param_type(params: &str) -> Option<&str> {
    let first = params.split(',').next()?.trim();
    let mut words = first.split_whitespace();
    let ty = words.next()?;
    (!ty.starts_with('$')).then(|| ty.trim_start_matches('?'))
}

fn typed_models<'a>(
    symbol: &resources::Symbol,
    methods: &[&str],
    ctx: &'a Context,
) -> BTreeSet<&'a str> {
    let mut out = BTreeSet::new();
    for m in symbol
        .methods
        .iter()
        .filter(|m| methods.is_empty() || methods.contains(&m.name.as_str()))
    {
        for param in m.params.split(',') {
            for word in param.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\\')) {
                if let Some((name, _)) = ctx.models.get_key_value(short(word)) {
                    out.insert(name.as_str());
                }
            }
        }
    }
    out
}

fn stem<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    name.strip_suffix(suffix).filter(|s| !s.is_empty())
}

fn request_model(name: &str) -> Option<&str> {
    let base = stem(name, "Request")?;
    for verb in [
        "Store", "Update", "Create", "Destroy", "Delete", "Edit", "Save", "Upsert", "Index", "Show",
    ] {
        if let Some(rest) = base.strip_prefix(verb)
            && !rest.is_empty()
        {
            return Some(rest);
        }
    }
    Some(base)
}

fn factory_model(text: &str) -> Option<String> {
    let at = text.find("$model")?;
    let rest = &text[at..];
    let end = rest.find(';')?;
    let value = &rest[..end];
    let class = value.split('=').nth(1)?.trim().strip_suffix("::class")?;
    Some(short(class).to_owned())
}

pub fn find(rel_path: &str, text: &str, ctx: &Context) -> Found {
    let mut found = Found::default();
    let rel_path = rel_path.replace('\\', "/");
    if rel_path.ends_with(".blade.php") {
        if let Some(name) = view_name(&rel_path) {
            let end = text.find(['\r', '\n']).unwrap_or(text.len());
            found.markers.push((view_key(&name), 0, end));
            found.definitions.push((view_file_key(&name), 0, end));
        }
        return found;
    }
    if !rel_path.ends_with(".php") {
        return found;
    }
    let views = view_refs(text);
    for (view, start, end) in &views {
        found.definitions.push((view_key(view), *start, *end));
    }
    if rel_path.starts_with("routes/") {
        for route in parse_routes(text) {
            if let Some((class, method)) = &route.action {
                found
                    .definitions
                    .push((action_key(class, method), route.start, route.end));
            }
        }
        return found;
    }
    if rel_path.starts_with("tests/") {
        for (value, start, end) in strings(text) {
            for route in &ctx.routes {
                let named = !route.name.is_empty() && route.name == value;
                let hits_uri = route.uri.len() > 1
                    && value.contains('/')
                    && !value.contains(' ')
                    && uri_matches(&route.uri, &value);
                if named || hits_uri {
                    found
                        .definitions
                        .push((format!("{KEY}action:{}", route.action), start, end));
                }
            }
        }
        return found;
    }
    if rel_path.starts_with("database/migrations/") {
        for (table, start, end) in schema_tables(text) {
            if let Some(model) = ctx.model_of_table(&table) {
                found.member(model_key(model), start, end);
            }
        }
        return found;
    }
    if !(rel_path.starts_with("app/") || rel_path.starts_with("database/")) {
        return found;
    }
    let Some(class) = parse_class(text) else {
        let enumeration = resources::scan_text(&rel_path, text)
            .symbols
            .into_iter()
            .find(|s| s.keyword == "enum");
        if let Some(e) = enumeration
            && let Some(listed) = ctx.cast_models.get(&e.name)
            && let Some(at) = text.find(&format!("enum {}", e.name))
        {
            let start = at + "enum ".len();
            for model in listed {
                found.member(model_key(model), start, start + e.name.len());
            }
        }
        return found;
    };
    let name = class.name.clone();
    let (cs, ce) = (class.start, class.end);
    let (namespace, imported) = imports(text);
    let symbol = resources::scan_text(&rel_path, text)
        .symbols
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_default();
    let method_at = |method: &str| {
        class
            .methods
            .iter()
            .find(|m| m.name == method)
            .map(|m| (m.start, m.start + m.name.len()))
    };
    let mut models: BTreeSet<String> = BTreeSet::new();
    if is_model(&class) {
        found.member(model_key(&name), cs, ce);
        for method in &symbol.methods {
            if crate::scope_name(method).is_some()
                && let Some((s, e)) = method_at(&method.name)
            {
                found.definitions.push((model_key(&name), s, e));
            }
        }
        for method in &class.methods {
            if let Some((_, Some(other))) = relation_of(&text[method.body.0..method.body.1])
                && other != name
                && ctx.is_model(short(&other))
            {
                found.definitions.push((model_key(short(&other)), cs, ce));
            }
        }
        return found;
    }
    let in_dir = |dir: &str| rel_path.starts_with(dir);
    if in_dir("database/factories/") {
        if let Some(model) =
            factory_model(text).or_else(|| stem(&name, "Factory").map(str::to_owned))
            && ctx.is_model(&model)
        {
            models.insert(model);
        }
    } else if in_dir("database/seeders/") {
        if let Some(model) = stem(&name, "Seeder").filter(|m| ctx.is_model(m)) {
            models.insert(model.to_owned());
        }
        for model in ctx.models.keys() {
            if text.contains(&format!("{model}::")) {
                models.insert(model.clone());
            }
        }
    } else if in_dir("app/Policies/") {
        match stem(&name, "Policy").filter(|m| ctx.is_model(m)) {
            Some(model) => {
                models.insert(model.to_owned());
            }
            None => models.extend(
                typed_models(&symbol, &[], ctx)
                    .into_iter()
                    .map(str::to_owned),
            ),
        }
    } else if name.ends_with("Observer") {
        match stem(&name, "Observer").filter(|m| ctx.is_model(m)) {
            Some(model) => {
                models.insert(model.to_owned());
            }
            None => models.extend(
                typed_models(&symbol, &[], ctx)
                    .into_iter()
                    .map(str::to_owned),
            ),
        }
    } else if in_dir("app/Http/Resources/") {
        if let Some(model) = stem(&name, "Resource")
            .or_else(|| stem(&name, "Collection"))
            .filter(|m| ctx.is_model(m))
        {
            models.insert(model.to_owned());
        }
    } else if in_dir("app/Http/Requests/") {
        if let Some(model) = request_model(&name).filter(|m| ctx.is_model(m)) {
            models.insert(model.to_owned());
        }
    } else if in_dir("app/Http/Controllers/") {
        if let Some(model) = stem(&name, "Controller").filter(|m| ctx.is_model(m)) {
            models.insert(model.to_owned());
        }
        for method in class.methods.iter().filter(|m| m.public) {
            if method.name.starts_with("__") {
                continue;
            }
            found.markers.push((
                action_key(&name, &method.name),
                method.start,
                method.start + method.name.len(),
            ));
        }
    } else if in_dir("app/Events/") {
        let fqn = if namespace.is_empty() {
            name.clone()
        } else {
            format!("{namespace}\\{name}")
        };
        found.member(event_key(&fqn), cs, ce);
        if let Some(listed) = ctx.event_models.get(&name) {
            models.extend(listed.iter().cloned());
        }
    } else if in_dir("app/Listeners/") {
        let handle = symbol.methods.iter().find(|m| m.name == "handle");
        if let Some(event) = handle.and_then(|m| first_param_type(&m.params)) {
            let fqn = resolve(event, &namespace, &imported);
            found.member(event_key(&fqn), cs, ce);
            if let Some(listed) = ctx.event_models.get(short(&fqn)) {
                models.extend(listed.iter().cloned());
            }
        }
    }
    if (in_dir("app/Jobs/") || in_dir("app/Mail/") || in_dir("app/Notifications/"))
        && let Some(listed) = ctx.event_models.get(&name)
    {
        models.extend(listed.iter().cloned());
    }
    if let Some(listed) = ctx.cast_models.get(&name) {
        models.extend(listed.iter().cloned());
    }
    let component = symbol.extends.iter().any(|e| short(e) == "Component");
    let renders_views = component || in_dir("app/Mail/") || in_dir("app/Notifications/");
    if renders_views {
        let mut seen = BTreeSet::new();
        for (view, _, _) in &views {
            if seen.insert(view.clone()) {
                found.markers.push((view_file_key(view), cs, ce));
            }
        }
        if seen.is_empty()
            && component
            && let Some(view) = livewire_view(&rel_path)
        {
            found.markers.push((view_file_key(&view), cs, ce));
            found.definitions.push((view_key(&view), cs, ce));
        }
    }
    for model in models {
        found.member(model_key(&model), cs, ce);
    }
    found
}

pub const ZONES: [&str; 5] = ["src", "tests", "routes", "migrations", "views"];

pub fn zone_of(rel_path: &str) -> &'static str {
    let rel_path = rel_path.trim_start_matches('/');
    if rel_path.starts_with("tests/") {
        "tests"
    } else if rel_path.starts_with("routes/") {
        "routes"
    } else if rel_path.starts_with("database/migrations/") {
        "migrations"
    } else if rel_path.starts_with("resources/views/") {
        "views"
    } else {
        "src"
    }
}

pub fn zoned(found: Found, rel_path: &str) -> Found {
    let own = zone_of(&rel_path.replace('\\', "/"));
    let definitions = found
        .definitions
        .into_iter()
        .map(|(key, s, e)| (format!("{key}|{own}"), s, e))
        .collect();
    let markers = found
        .markers
        .into_iter()
        .flat_map(|(key, s, e)| ZONES.map(|zone| (format!("{key}|{zone}"), s, e)))
        .collect();
    Found {
        markers,
        definitions,
    }
}

pub fn event_models(
    scan: &resources::Scan,
    models: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for symbol in scan.symbols.iter().filter(|s| {
        let path = s.path.replace('\\', "/");
        [
            "app/Events/",
            "app/Jobs/",
            "app/Mail/",
            "app/Notifications/",
        ]
        .iter()
        .any(|dir| path.contains(dir))
    }) {
        let Some(ctor) = symbol.methods.iter().find(|m| m.name == "__construct") else {
            continue;
        };
        let mut listed: Vec<String> = Vec::new();
        for word in ctor
            .params
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\\'))
        {
            let name = short(word);
            if models.contains_key(name) && !listed.iter().any(|m| m == name) {
                listed.push(name.to_owned());
            }
        }
        if !listed.is_empty() {
            out.insert(symbol.name.clone(), listed);
        }
    }
    out
}

pub fn cast_models(
    scan: &resources::Scan,
    models: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for symbol in scan.symbols.iter().filter(|s| models.contains_key(&s.name)) {
        let Some(casts) = symbol.methods.iter().find(|m| m.name == "casts") else {
            continue;
        };
        for piece in casts
            .body
            .split("::class")
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .skip(1)
        {
            let class = piece
                .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\\'))
                .next()
                .unwrap_or_default();
            let class = short(class);
            if class.is_empty() || models.contains_key(class) {
                continue;
            }
            let listed = out.entry(class.to_owned()).or_default();
            if !listed.contains(&symbol.name) {
                listed.push(symbol.name.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            models: [
                ("Order", "orders"),
                ("User", "users"),
                ("Product", "products"),
            ]
            .into_iter()
            .map(|(m, t)| (m.to_owned(), t.to_owned()))
            .collect(),
            routes: vec![
                RouteRef {
                    name: "orders.store".into(),
                    uri: "/orders".into(),
                    action: "OrderController@store".into(),
                },
                RouteRef {
                    name: String::new(),
                    uri: "/orders/{order}".into(),
                    action: "OrderController@show".into(),
                },
            ],
            event_models: [
                ("OrderShipped".to_owned(), vec!["Order".to_owned()]),
                ("ShipOrder".to_owned(), vec!["Order".to_owned()]),
            ]
            .into_iter()
            .collect(),
            cast_models: [("OrderStatus".to_owned(), vec!["Order".to_owned()])]
                .into_iter()
                .collect(),
        }
    }

    fn keys(list: &[(String, usize, usize)], text: &str) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, s, e)| (k.clone(), text[*s..*e].to_owned()))
            .collect()
    }

    fn pair(key: &str, at: &str) -> (String, String) {
        (key.to_owned(), at.to_owned())
    }

    #[test]
    fn a_model_lists_what_belongs_to_it() {
        let text = "<?php\nnamespace App\\Models;\n\nclass Order extends Model\n{\n    public function scopePaid($q) { return $q; }\n\n    #[Scope]\n    protected function open($q) { return $q; }\n\n    public function user(): BelongsTo\n    {\n        return $this->belongsTo(User::class);\n    }\n\n    public function notes()\n    {\n        return 1;\n    }\n}\n";
        let found = find("app/Models/Order.php", text, &ctx());
        assert_eq!(
            keys(&found.markers, text),
            [pair("laravel-related#model:Order", "Order")]
        );
        assert_eq!(
            keys(&found.definitions, text),
            [
                pair("laravel-related#model:Order", "Order"),
                pair("laravel-related#model:Order", "scopePaid"),
                pair("laravel-related#model:Order", "open"),
                pair("laravel-related#model:User", "Order"),
            ]
        );
    }

    #[test]
    fn the_files_of_a_model_join_its_list() {
        let member = |path: &str, text: &str| -> Vec<String> {
            let found = find(path, text, &ctx());
            assert_eq!(found.markers, found.definitions, "{path}");
            found.markers.into_iter().map(|(k, _, _)| k).collect()
        };
        let order = vec!["laravel-related#model:Order".to_owned()];
        assert_eq!(
            member(
                "database/factories/OrderFactory.php",
                "<?php\nclass OrderFactory extends Factory\n{\n}\n"
            ),
            order
        );
        assert_eq!(
            member(
                "database/factories/PurchaseFactory.php",
                "<?php\nclass PurchaseFactory extends Factory\n{\n    protected $model = Order::class;\n}\n"
            ),
            order
        );
        assert_eq!(
            member(
                "database/seeders/ShopSeeder.php",
                "<?php\nclass ShopSeeder extends Seeder\n{\n    public function run(): void\n    {\n        Product::factory(3)->create();\n        Order::factory(2)->create();\n    }\n}\n"
            ),
            [
                "laravel-related#model:Order",
                "laravel-related#model:Product"
            ]
        );
        assert_eq!(
            member(
                "app/Policies/OrderPolicy.php",
                "<?php\nclass OrderPolicy\n{\n}\n"
            ),
            order
        );
        assert_eq!(
            member(
                "app/Policies/SalesPolicy.php",
                "<?php\nclass SalesPolicy\n{\n    public function view(User $user, Order $order): bool { return true; }\n}\n"
            ),
            ["laravel-related#model:Order", "laravel-related#model:User"]
        );
        assert_eq!(
            member(
                "app/Observers/AuditObserver.php",
                "<?php\nclass AuditObserver\n{\n    public function created(Order $order): void {}\n}\n"
            ),
            order
        );
        assert_eq!(
            member(
                "app/Http/Resources/OrderCollection.php",
                "<?php\nclass OrderCollection extends ResourceCollection\n{\n}\n"
            ),
            order
        );
        assert_eq!(
            member(
                "app/Http/Requests/StoreOrderRequest.php",
                "<?php\nclass StoreOrderRequest extends FormRequest\n{\n}\n"
            ),
            order
        );
        assert!(
            member(
                "app/Http/Resources/HealthResource.php",
                "<?php\nclass HealthResource extends JsonResource\n{\n}\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn migrations_mark_each_table_they_touch() {
        let text = "<?php\nreturn new class extends Migration\n{\n    public function up(): void\n    {\n        Schema::create('orders', function (Blueprint $table) {});\n        Schema::table(\"users\", function (Blueprint $table) {});\n        Schema::create('jobs', function (Blueprint $table) {});\n    }\n};\n";
        let found = find(
            "database/migrations/2026_01_01_create_orders.php",
            text,
            &ctx(),
        );
        assert_eq!(
            keys(&found.markers, text),
            [
                pair("laravel-related#model:Order", "orders"),
                pair("laravel-related#model:User", "users"),
            ]
        );
        assert_eq!(found.markers, found.definitions);
    }

    #[test]
    fn controller_methods_lead_to_their_routes_and_tests() {
        let text = "<?php\nnamespace App\\Http\\Controllers;\n\nclass OrderController extends Controller\n{\n    public function store() { return view('orders.created'); }\n\n    public function show() {}\n\n    private function helper() {}\n\n    public function __construct() {}\n}\n";
        let found = find("app/Http/Controllers/OrderController.php", text, &ctx());
        assert_eq!(
            keys(&found.markers, text),
            [
                pair("laravel-related#action:OrderController@store", "store"),
                pair("laravel-related#action:OrderController@show", "show"),
                pair("laravel-related#model:Order", "OrderController"),
            ]
        );
        assert!(keys(&found.definitions, text).contains(&pair(
            "laravel-related#view:orders.created",
            "orders.created"
        )));

        let routes = "<?php\nuse App\\Http\\Controllers\\OrderController;\nRoute::post('/orders', [OrderController::class, 'store'])->name('orders.store');\nRoute::view('/about', 'pages.about');\n";
        let found = find("routes/web.php", routes, &ctx());
        let defs = keys(&found.definitions, routes);
        assert!(
            defs.contains(&pair("laravel-related#view:pages.about", "pages.about")),
            "{defs:?}"
        );
        assert!(
            defs.iter()
                .any(|(k, _)| k == "laravel-related#action:OrderController@store"),
            "{defs:?}"
        );
        assert!(found.markers.is_empty());

        let test = "<?php\nit('stores', function () {\n    $this->post(route('orders.store'), []);\n    $this->get('/orders/7');\n    $this->get('/nowhere');\n});\n";
        let found = find("tests/Feature/OrderTest.php", test, &ctx());
        assert_eq!(
            keys(&found.definitions, test),
            [
                pair(
                    "laravel-related#action:OrderController@store",
                    "orders.store"
                ),
                pair("laravel-related#action:OrderController@show", "/orders/7"),
            ]
        );
    }

    #[test]
    fn views_lead_to_what_renders_them_and_back() {
        let blade = "<div>\n    {{ $order->id }}\n</div>\n";
        let found = find("resources/views/orders/show.blade.php", blade, &ctx());
        assert_eq!(
            keys(&found.markers, blade),
            [pair("laravel-related#view:orders.show", "<div>")]
        );
        assert_eq!(
            keys(&found.definitions, blade),
            [pair("laravel-related#view-file:orders.show", "<div>")]
        );

        let livewire = "<?php\nnamespace App\\Livewire;\n\nclass OrderTable extends Component\n{\n    public function render()\n    {\n        return view('livewire.order-table');\n    }\n}\n";
        let found = find("app/Livewire/OrderTable.php", livewire, &ctx());
        assert_eq!(
            keys(&found.markers, livewire),
            [pair(
                "laravel-related#view-file:livewire.order-table",
                "OrderTable"
            )]
        );
        assert_eq!(
            keys(&found.definitions, livewire),
            [pair(
                "laravel-related#view:livewire.order-table",
                "livewire.order-table"
            )]
        );

        let convention =
            "<?php\nnamespace App\\Livewire\\Shop;\n\nclass CartSummary extends Component\n{\n}\n";
        let found = find("app/Livewire/Shop/CartSummary.php", convention, &ctx());
        assert_eq!(
            keys(&found.markers, convention),
            [pair(
                "laravel-related#view-file:livewire.shop.cart-summary",
                "CartSummary"
            )]
        );
        assert_eq!(
            keys(&found.definitions, convention),
            [pair(
                "laravel-related#view:livewire.shop.cart-summary",
                "CartSummary"
            )]
        );

        let mail = "<?php\nclass OrderShippedMail extends Mailable\n{\n    public function content(): Content\n    {\n        return new Content(view: 'mail.shipped');\n    }\n}\n";
        let found = find("app/Mail/OrderShippedMail.php", mail, &ctx());
        assert_eq!(
            keys(&found.markers, mail),
            [pair(
                "laravel-related#view-file:mail.shipped",
                "OrderShippedMail"
            )]
        );
    }

    #[test]
    fn events_and_their_listeners_join_the_model() {
        let event = "<?php\nnamespace App\\Events;\n\nclass OrderShipped\n{\n    public function __construct(public Order $order) {}\n}\n";
        let found = find("app/Events/OrderShipped.php", event, &ctx());
        assert_eq!(
            keys(&found.markers, event),
            [
                pair(
                    "laravel-related#event:App\\Events\\OrderShipped",
                    "OrderShipped"
                ),
                pair("laravel-related#model:Order", "OrderShipped"),
            ]
        );
        let listener = "<?php\nnamespace App\\Listeners;\n\nuse App\\Events\\OrderShipped;\n\nclass SendShipmentNotice\n{\n    public function handle(OrderShipped $event): void {}\n}\n";
        let found = find("app/Listeners/SendShipmentNotice.php", listener, &ctx());
        assert_eq!(
            keys(&found.markers, listener),
            [
                pair(
                    "laravel-related#event:App\\Events\\OrderShipped",
                    "SendShipmentNotice"
                ),
                pair("laravel-related#model:Order", "SendShipmentNotice"),
            ]
        );
        assert_eq!(found.markers, found.definitions);
    }

    #[test]
    fn keys_carry_the_area_of_the_file_that_defines_them() {
        let text = "<?php\nreturn new class extends Migration\n{\n    public function up(): void\n    {\n        Schema::create('orders', function (Blueprint $table) {});\n    }\n};\n";
        let path = "database/migrations/2026_01_01_create_orders.php";
        let found = zoned(find(path, text, &ctx()), path);
        assert_eq!(
            found
                .definitions
                .iter()
                .map(|(k, _, _)| k.as_str())
                .collect::<Vec<_>>(),
            ["laravel-related#model:Order|migrations"]
        );
        assert_eq!(
            found
                .markers
                .iter()
                .map(|(k, _, _)| k.as_str())
                .collect::<Vec<_>>(),
            [
                "laravel-related#model:Order|src",
                "laravel-related#model:Order|tests",
                "laravel-related#model:Order|routes",
                "laravel-related#model:Order|migrations",
                "laravel-related#model:Order|views",
            ]
        );
        assert_eq!(zone_of("resources/views/shop/index.blade.php"), "views");
        assert_eq!(zone_of("database/factories/OrderFactory.php"), "src");
    }

    #[test]
    fn jobs_and_enum_casts_join_the_model() {
        let job = "<?php\nnamespace App\\Jobs;\n\nclass ShipOrder implements ShouldQueue\n{\n}\n";
        let found = find("app/Jobs/ShipOrder.php", job, &ctx());
        assert_eq!(
            keys(&found.markers, job),
            [pair("laravel-related#model:Order", "ShipOrder")]
        );
        let enumeration = "<?php\nnamespace App\\Enums;\n\nenum OrderStatus: string\n{\n    case Paid = 'paid';\n}\n";
        let found = find("app/Enums/OrderStatus.php", enumeration, &ctx());
        assert_eq!(
            keys(&found.markers, enumeration),
            [pair("laravel-related#model:Order", "OrderStatus")]
        );
        let cast =
            "<?php\nnamespace App\\Casts;\n\nclass OrderStatus implements CastsAttributes\n{\n}\n";
        let found = find("app/Casts/OrderStatus.php", cast, &ctx());
        assert_eq!(
            keys(&found.markers, cast),
            [pair("laravel-related#model:Order", "OrderStatus")]
        );
        let mut scan = resources::Scan::default();
        scan.symbols.extend(
            resources::scan_text(
                "app/Models/Order.php",
                "<?php\nnamespace App\\Models;\nclass Order extends Model\n{\n    protected function casts(): array\n    {\n        return ['status' => OrderStatus::class, 'owner' => \\App\\Models\\User::class, 'paid_at' => 'datetime'];\n    }\n}\n",
            )
            .symbols,
        );
        let found = cast_models(&scan, &ctx().models);
        assert_eq!(found.keys().collect::<Vec<_>>(), ["OrderStatus"]);
        assert_eq!(found["OrderStatus"], ["Order"]);
    }

    #[test]
    fn reads_the_models_of_events_from_their_constructor() {
        let mut scan = resources::Scan::default();
        scan.symbols.extend(
            resources::scan_text(
                "app/Events/OrderShipped.php",
                "<?php\nnamespace App\\Events;\nclass OrderShipped\n{\n    public function __construct(public Order $order, public ?User $by = null) {}\n}\n",
            )
            .symbols,
        );
        let found = event_models(&scan, &ctx().models);
        assert_eq!(found["OrderShipped"], ["Order", "User"]);
    }
}
