use crate::casts::resolve_name;
use crate::directives;
use crate::index::Entry;
use crate::index::rules;
use crate::objects::{self, Members};
use crate::php::{Kind, Token, tokenize};
use crate::project::{Lines, join};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const BASE: &str = "vendor/laravel/framework/";

pub type Method = (String, String);

pub const RELATIONS: [(&str, &str); 11] = [
    ("hasOne", "HasOne"),
    ("hasMany", "HasMany"),
    ("hasOneThrough", "HasOneThrough"),
    ("hasManyThrough", "HasManyThrough"),
    ("belongsTo", "BelongsTo"),
    ("belongsToMany", "BelongsToMany"),
    ("morphOne", "MorphOne"),
    ("morphMany", "MorphMany"),
    ("morphTo", "MorphTo"),
    ("morphToMany", "MorphToMany"),
    ("morphedByMany", "MorphToMany"),
];

const COMPILER_INPUTS: [&str; 4] = ["$value", "$match", "$matches", "$template"];

const VALIDATOR_RULE_LISTS: [&str; 6] = [
    "implicitRules",
    "dependentRules",
    "excludeRules",
    "sizeRules",
    "numericRules",
    "fileRules",
];

#[derive(Debug, Default)]
pub struct Framework {
    pub installed: bool,
    psr4: Vec<(String, Vec<String>)>,
    pub rules: Vec<Entry>,
    pub directives: Vec<Entry>,
    pub casts: Vec<String>,
    pub casts_method: bool,
    pub middleware: Vec<Entry>,
    pub container: Vec<(String, String)>,
    pub facades: Vec<(String, String)>,
    pub collection: Vec<Method>,
    pub query: Vec<Method>,
    pub relations: BTreeMap<String, Vec<Method>>,
    pub factory: Vec<Method>,
    pub factory_method: Option<openidea_plugin::Location>,
    classes: BTreeMap<String, bool>,
    packages: BTreeSet<String>,
}

impl Framework {
    pub fn check<'a>(
        &mut self,
        exists: &dyn Fn(&str) -> bool,
        classes: impl IntoIterator<Item = &'a str>,
        packages: impl IntoIterator<Item = &'a str>,
    ) {
        for fqn in classes {
            let fqn = fqn.trim_start_matches('\\');
            if self.classes.contains_key(fqn) {
                continue;
            }
            let found = candidates(&self.psr4, fqn).iter().any(|path| exists(path));
            self.classes.insert(fqn.to_owned(), found);
        }
        for package in packages {
            if exists(&format!("vendor/{package}/composer.json")) {
                self.packages.insert(package.to_owned());
            }
        }
    }

    pub fn has_class(&self, fqn: &str) -> bool {
        !self.installed
            || self
                .classes
                .get(fqn.trim_start_matches('\\'))
                .copied()
                .unwrap_or(true)
    }

    pub fn has_package(&self, package: &str) -> bool {
        !self.installed || self.packages.contains(package)
    }
}

pub fn load(read: &dyn Fn(&str) -> Option<String>, root: &str) -> Framework {
    let Some(composer) = read(&format!("{BASE}composer.json")) else {
        return Framework::default();
    };
    let psr4 = psr4(&composer);
    let mut loader = Loader {
        read,
        psr4: &psr4,
        files: BTreeMap::new(),
    };
    let (casts, casts_method) = casts(&mut loader);
    let mut query = loader.methods("Illuminate\\Database\\Eloquent\\Builder");
    for method in loader.methods("Illuminate\\Database\\Query\\Builder") {
        add(&mut query, method);
    }
    let relations = RELATIONS
        .iter()
        .map(|(kind, class)| {
            let fqn = format!("Illuminate\\Database\\Eloquent\\Relations\\{class}");
            ((*kind).to_owned(), loader.methods(&fqn))
        })
        .collect();
    let framework = Framework {
        installed: true,
        rules: validation_rules(&mut loader, root),
        directives: blade_directives(&mut loader, root),
        casts,
        casts_method,
        middleware: middleware(&mut loader, root),
        container: container(&mut loader),
        facades: facades(&mut loader),
        collection: loader.methods("Illuminate\\Database\\Eloquent\\Collection"),
        query,
        relations,
        factory: loader.methods("Illuminate\\Database\\Eloquent\\Factories\\Factory"),
        factory_method: factory_method(&mut loader, root),
        classes: BTreeMap::new(),
        packages: BTreeSet::new(),
        psr4: Vec::new(),
    };
    Framework { psr4, ..framework }
}

fn psr4(composer: &str) -> Vec<(String, Vec<String>)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(composer) else {
        return Vec::new();
    };
    let Some(map) = json["autoload"]["psr-4"].as_object() else {
        return Vec::new();
    };
    let mut out: Vec<(String, Vec<String>)> = map
        .iter()
        .map(|(prefix, dirs)| {
            let dirs = match dirs {
                serde_json::Value::String(d) => vec![d.clone()],
                serde_json::Value::Array(list) => list
                    .iter()
                    .filter_map(|d| d.as_str().map(str::to_owned))
                    .collect(),
                _ => Vec::new(),
            };
            (prefix.clone(), dirs)
        })
        .collect();
    out.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
    out
}

fn candidates(psr4: &[(String, Vec<String>)], fqn: &str) -> Vec<String> {
    let fqn = fqn.trim_start_matches('\\');
    psr4.iter()
        .filter_map(|(prefix, dirs)| Some((fqn.strip_prefix(prefix.as_str())?, dirs)))
        .flat_map(|(rest, dirs)| {
            let rest = rest.replace('\\', "/");
            dirs.iter().map(move |dir| {
                let dir = dir.trim_end_matches('/');
                format!("{BASE}{dir}/{rest}.php")
            })
        })
        .collect()
}

struct Source {
    path: String,
    text: Rc<String>,
    members: Rc<Members>,
}

struct Loader<'a> {
    read: &'a dyn Fn(&str) -> Option<String>,
    psr4: &'a [(String, Vec<String>)],
    files: BTreeMap<String, Option<Rc<Source>>>,
}

impl Loader<'_> {
    fn source(&mut self, fqn: &str) -> Option<Rc<Source>> {
        let fqn = fqn.trim_start_matches('\\');
        if let Some(hit) = self.files.get(fqn) {
            return hit.clone();
        }
        let found = candidates(self.psr4, fqn).into_iter().find_map(|path| {
            let text = (self.read)(&path)?;
            let members = objects::read(&text)?;
            Some(Rc::new(Source {
                path,
                text: Rc::new(text),
                members: Rc::new(members),
            }))
        });
        self.files.insert(fqn.to_owned(), found.clone());
        found
    }

    fn walk(
        &mut self,
        fqn: &str,
        depth: u8,
        seen: &mut BTreeSet<String>,
        out: &mut Vec<Rc<Source>>,
    ) {
        let fqn = fqn.trim_start_matches('\\');
        if depth > 8 || !seen.insert(fqn.to_ascii_lowercase()) {
            return;
        }
        let Some(source) = self.source(fqn) else {
            return;
        };
        out.push(source.clone());
        let header = header(&source.text);
        for name in &source.members.traits {
            let used = resolve_name(header, name);
            self.walk(&used, depth + 1, seen, out);
        }
        if let Some(parent) = &source.members.extends {
            let parent = resolve_name(header, parent);
            self.walk(&parent, depth + 1, seen, out);
        }
    }

    fn hierarchy(&mut self, fqn: &str) -> Vec<Rc<Source>> {
        let mut out = Vec::new();
        self.walk(fqn, 0, &mut BTreeSet::new(), &mut out);
        out
    }

    fn methods(&mut self, fqn: &str) -> Vec<Method> {
        let mut out = Vec::new();
        for source in self.hierarchy(fqn) {
            for (name, signature, is_static) in &source.members.methods {
                if !is_static {
                    add(&mut out, (name.clone(), signature.clone()));
                }
            }
        }
        out
    }
}

fn header(text: &str) -> &str {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let declares = line
            .split_whitespace()
            .find(|w| !matches!(*w, "abstract" | "final" | "readonly"))
            .is_some_and(|w| matches!(w, "class" | "trait" | "interface" | "enum"));
        if declares {
            return &text[..at];
        }
        at += line.len();
    }
    text
}

fn add(methods: &mut Vec<Method>, method: Method) {
    if !methods
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(&method.0))
    {
        methods.push(method);
    }
}

fn offset_of(members: &Members, name: &str) -> usize {
    members
        .offsets
        .iter()
        .find(|(n, _)| n == name)
        .map_or(0, |(_, at)| *at)
}

fn snake(studly: &str) -> String {
    let mut out = String::new();
    for (i, c) in studly.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn lcfirst(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

fn params(signature: &str) -> Vec<String> {
    let inner = signature.split_once('(').map_or("", |(_, rest)| {
        rest.rsplit_once(')').map_or(rest, |(p, _)| p)
    });
    inner
        .split(',')
        .filter_map(|p| {
            let var = p.split_whitespace().find(|w| w.starts_with('$'))?;
            Some(var.trim_end_matches(['=', ')']).to_owned())
        })
        .collect()
}

fn entry(root: &str, source: &Source, key: String, detail: String, at: usize, len: usize) -> Entry {
    let lines = Lines::new(&source.text);
    Entry::new(
        key,
        Some(detail),
        &join(root, &source.path),
        lines.range(&source.text, at, at + len),
    )
}

fn validation_rules(loader: &mut Loader, root: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let known = |rule: &str, takes_parameters: bool| {
        rules::RULES
            .iter()
            .chain(rules::EXTRA)
            .find(|(n, _)| *n == rule)
            .map(|(_, d)| (*d).to_owned())
            .unwrap_or_else(|| {
                if takes_parameters {
                    format!("{rule}:…")
                } else {
                    rule.to_owned()
                }
            })
    };
    for source in loader.hierarchy("Illuminate\\Validation\\Concerns\\ValidatesAttributes") {
        let all = source
            .members
            .methods
            .iter()
            .map(|(n, s, _)| (n, s))
            .chain(source.members.hidden.iter().map(|(n, s)| (n, s)));
        for (name, signature) in all {
            let Some(rest) = name.strip_prefix("validate") else {
                continue;
            };
            let args = params(signature);
            if !rest.starts_with(|c: char| c.is_ascii_uppercase())
                || args.first().is_some_and(|a| a != "$attribute")
            {
                continue;
            }
            let rule = snake(rest);
            if out.iter().any(|e| e.key == rule) {
                continue;
            }
            let detail = known(&rule, args.iter().any(|a| a == "$parameters"));
            let at = offset_of(&source.members, name);
            out.push(entry(root, &source, rule, detail, at, name.len()));
        }
    }
    if let Some(source) = loader.source("Illuminate\\Validation\\Validator") {
        let tokens = tokenize(&source.text, 0, source.text.len());
        for list in VALIDATOR_RULE_LISTS {
            for (value, at) in property_strings(&tokens, list) {
                let rule = snake(&value);
                if out.iter().any(|e| e.key == rule) {
                    continue;
                }
                let detail = known(&rule, false);
                out.push(entry(root, &source, rule, detail, at, value.len()));
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

fn blade_directives(loader: &mut Loader, root: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for source in loader.hierarchy("Illuminate\\View\\Compilers\\BladeCompiler") {
        let all = source
            .members
            .methods
            .iter()
            .map(|(n, s, _)| (n, s))
            .chain(source.members.hidden.iter().map(|(n, s)| (n, s)));
        for (name, signature) in all {
            let Some(rest) = name.strip_prefix("compile") else {
                continue;
            };
            let args = params(signature);
            if rest.is_empty()
                || args.len() > 1
                || args
                    .first()
                    .is_some_and(|a| COMPILER_INPUTS.contains(&a.as_str()))
            {
                continue;
            }
            if out.iter().any(|e| e.key.eq_ignore_ascii_case(rest)) {
                continue;
            }
            let (key, detail) = match directives::BUILTIN
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(rest))
            {
                Some((n, d)) => ((*n).to_owned(), (*d).to_owned()),
                None => {
                    let key = lcfirst(rest);
                    let detail = if args.is_empty() {
                        format!("@{key}")
                    } else {
                        format!("@{key}(…)")
                    };
                    (key, detail)
                }
            };
            let at = offset_of(&source.members, name);
            out.push(entry(root, &source, key, detail, at, name.len()));
        }
    }
    if !out.is_empty() {
        for (name, detail) in directives::BUILTIN {
            if directives::SYNTAX.contains(name) && !out.iter().any(|e| e.key == *name) {
                out.push(Entry::new(
                    *name,
                    Some((*detail).to_owned()),
                    "",
                    openidea_plugin::Range::default(),
                ));
            }
        }
    }
    out
}

fn factory_method(loader: &mut Loader, root: &str) -> Option<openidea_plugin::Location> {
    let source = loader.source("Illuminate\\Database\\Eloquent\\Factories\\HasFactory")?;
    let at = source
        .members
        .offsets
        .iter()
        .find(|(n, _)| n == "factory")
        .map(|(_, at)| *at)?;
    let lines = Lines::new(&source.text);
    Some(openidea_plugin::Location {
        path: join(root, &source.path),
        range: lines.range(&source.text, at, at + "factory".len()),
    })
}

fn casts(loader: &mut Loader) -> (Vec<String>, bool) {
    let Some(source) = loader.source("Illuminate\\Database\\Eloquent\\Concerns\\HasAttributes")
    else {
        return (Vec::new(), false);
    };
    let has_method = source
        .members
        .methods
        .iter()
        .map(|(n, _, _)| n)
        .chain(source.members.hidden.iter().map(|(n, _)| n))
        .any(|n| n == "casts");
    let tokens = tokenize(&source.text, 0, source.text.len());
    let mut types: Vec<String> = property_strings(&tokens, "primitiveCastTypes")
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    if types.is_empty()
        && let Some((open, close)) = method_body(&tokens, "castAttribute")
    {
        for i in open..close {
            if matches!(&tokens[i].kind, Kind::Ident(w) if w == "case")
                && let Some(Kind::Str { value, .. }) = tokens.get(i + 1).map(|t| &t.kind)
                && !types.contains(value)
            {
                types.push(value.clone());
            }
        }
    }
    (types, has_method)
}

fn middleware(loader: &mut Loader, root: &str) -> Vec<Entry> {
    let Some(source) = loader.source("Illuminate\\Foundation\\Configuration\\Middleware") else {
        return Vec::new();
    };
    let tokens = tokenize(&source.text, 0, source.text.len());
    let mut out = Vec::new();
    for (method, group) in [("defaultAliases", false), ("getMiddlewareGroups", true)] {
        let Some((open, close)) = method_body(&tokens, method) else {
            continue;
        };
        for (key, at, class) in keyed(&tokens, open, close) {
            let detail = if group {
                "middleware group".to_owned()
            } else {
                class
                    .as_deref()
                    .map_or("middleware", |c| c.rsplit('\\').next().unwrap_or(c))
                    .to_owned()
            };
            let len = key.len();
            out.push(entry(root, &source, key, detail, at, len));
        }
    }
    out
}

fn facades(loader: &mut Loader) -> Vec<(String, String)> {
    let Some(source) = loader.source("Illuminate\\Support\\Facades\\Facade") else {
        return Vec::new();
    };
    let tokens = tokenize(&source.text, 0, source.text.len());
    let Some((open, close)) = method_body(&tokens, "defaultAliases") else {
        return Vec::new();
    };
    let head = header(&source.text);
    keyed(&tokens, open, close)
        .into_iter()
        .filter_map(|(alias, _, class)| Some((alias, crate::casts::resolve_name(head, &class?))))
        .collect()
}

fn container(loader: &mut Loader) -> Vec<(String, String)> {
    let Some(source) = loader.source("Illuminate\\Foundation\\Application") else {
        return Vec::new();
    };
    let tokens = tokenize(&source.text, 0, source.text.len());
    let Some((open, close)) = method_body(&tokens, "registerCoreContainerAliases") else {
        return Vec::new();
    };
    keyed(&tokens, open, close)
        .into_iter()
        .map(|(key, _, class)| {
            let class = match class.as_deref() {
                Some("self" | "static") | None => "Illuminate\\Foundation\\Application".to_owned(),
                Some(c) => c.trim_start_matches('\\').to_owned(),
            };
            (key, class)
        })
        .collect()
}

fn ident(tokens: &[Token], i: usize) -> Option<&str> {
    match &tokens.get(i)?.kind {
        Kind::Ident(s) => Some(s),
        _ => None,
    }
}

fn closing(tokens: &[Token], open: usize) -> usize {
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

fn method_body(tokens: &[Token], name: &str) -> Option<(usize, usize)> {
    let at = (0..tokens.len())
        .find(|&i| ident(tokens, i) == Some("function") && ident(tokens, i + 1) == Some(name))?;
    let open = (at..tokens.len()).find(|&i| tokens[i].kind == Kind::Open('{'))?;
    Some((open, closing(tokens, open)))
}

fn property_strings(tokens: &[Token], property: &str) -> Vec<(String, usize)> {
    let Some(at) = (0..tokens.len()).find(|&i| {
        matches!(&tokens[i].kind, Kind::Var(v) if v == property)
            && tokens.get(i + 1).is_some_and(|t| t.kind == Kind::Assign)
    }) else {
        return Vec::new();
    };
    let Some(open) = tokens
        .get(at + 2)
        .filter(|t| t.kind == Kind::Open('['))
        .map(|_| at + 2)
    else {
        return Vec::new();
    };
    let close = closing(tokens, open);
    tokens[open..close]
        .iter()
        .filter_map(|t| match &t.kind {
            Kind::Str { value, .. } => Some((value.clone(), t.start + 1)),
            _ => None,
        })
        .collect()
}

fn keyed(tokens: &[Token], from: usize, to: usize) -> Vec<(String, usize, Option<String>)> {
    let Some(open) = (from..to).find(|&i| tokens[i].kind == Kind::Open('[')) else {
        return Vec::new();
    };
    let close = closing(tokens, open).min(to);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = open;
    while i < close {
        match &tokens[i].kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth = depth.saturating_sub(1),
            Kind::Str { value, .. }
                if depth == 1
                    && tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == Kind::DoubleArrow) =>
            {
                let mut class = None;
                let mut j = i + 2;
                let mut inner = 0usize;
                while j < close {
                    match &tokens[j].kind {
                        Kind::Open(_) => inner += 1,
                        Kind::Close(_) if inner == 0 => break,
                        Kind::Close(_) => inner -= 1,
                        Kind::Comma if inner == 0 => break,
                        Kind::Ident(name)
                            if class.is_none()
                                && tokens
                                    .get(j + 1)
                                    .is_some_and(|t| t.kind == Kind::DoubleColon)
                                && ident(tokens, j + 2) == Some("class") =>
                        {
                            class = Some(name.clone());
                        }
                        _ => {}
                    }
                    j += 1;
                }
                out.push((value.clone(), tokens[i].start + 1, class));
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn reader(files: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: HashMap<String, String> = files
            .iter()
            .map(|(p, t)| (format!("{BASE}{p}"), (*t).to_owned()))
            .collect();
        move |path: &str| map.get(path).cloned()
    }

    const COMPOSER: &str = r#"{"autoload": {"psr-4": {"Illuminate\\": "src/Illuminate/", "Illuminate\\Support\\": ["src/Illuminate/Macroable/", "src/Illuminate/Collections/"]}}}"#;

    #[test]
    fn without_the_framework_nothing_is_installed() {
        let fw = load(&reader(&[]), "/p");
        assert!(!fw.installed && fw.rules.is_empty() && fw.directives.is_empty());
    }

    #[test]
    fn resolves_classes_through_every_psr4_directory() {
        let map = psr4(COMPOSER);
        assert_eq!(
            candidates(&map, "Illuminate\\Support\\Collection"),
            [
                "vendor/laravel/framework/src/Illuminate/Macroable/Collection.php",
                "vendor/laravel/framework/src/Illuminate/Collections/Collection.php",
                "vendor/laravel/framework/src/Illuminate/Support/Collection.php",
            ]
        );
    }

    #[test]
    fn reads_rules_directives_casts_and_aliases_from_the_sources() {
        let attributes = "<?php\nnamespace Illuminate\\Validation\\Concerns;\n\ntrait ValidatesAttributes\n{\n    public function validateRequired($attribute, $value) { return true; }\n    public function validateHexColor($attribute, $value) { return true; }\n    public function validateBetween($attribute, $value, $parameters) { return true; }\n    protected function validateHelper($value) {}
    protected function validateCurrentPassword($attribute, $value, $parameters) {}\n    public function validateAttribute($x) {}\n}\n";
        let validator = "<?php\nnamespace Illuminate\\Validation;\n\nclass Validator\n{\n    protected $implicitRules = ['Accepted', 'Required', 'Sometimes'];\n}\n";
        let compiler = "<?php\nnamespace Illuminate\\View\\Compilers;\n\nclass BladeCompiler\n{\n    use Concerns\\CompilesConditionals;\n\n    protected function compileEchos($value) {}\n    protected function compileStatement($match) {}\n    public function compileString($value) {}\n}\n";
        let conditionals = "<?php\nnamespace Illuminate\\View\\Compilers\\Concerns;\n\ntrait CompilesConditionals\n{\n    protected function compileEndAuth() {}\n    protected function compileBool($condition) {}\n    protected function compileIf($expression) {}\n    protected function compileClassComponentOpening(string $a, string $b) {}\n}\n";
        let attributes_trait = "<?php\nnamespace Illuminate\\Database\\Eloquent\\Concerns;\n\ntrait HasAttributes\n{\n    protected static $primitiveCastTypes = [\n        'array',\n        'bool',\n        'hashed',\n    ];\n}\n";
        let middleware = "<?php\nnamespace Illuminate\\Foundation\\Configuration;\n\nclass Middleware\n{\n    public function getMiddlewareGroups()\n    {\n        $middleware = [\n            'web' => array_values(array_filter([\\Foo::class, $x ? 'a' : null])),\n            'api' => [],\n        ];\n    }\n\n    protected function defaultAliases()\n    {\n        $aliases = [\n            'auth' => \\Illuminate\\Auth\\Middleware\\Authenticate::class,\n            'throttle' => $this->redis\n                ? \\Illuminate\\Routing\\Middleware\\ThrottleRequestsWithRedis::class\n                : \\Illuminate\\Routing\\Middleware\\ThrottleRequests::class,\n        ];\n    }\n}\n";
        let application = "<?php\nnamespace Illuminate\\Foundation;\n\nclass Application\n{\n    public function registerCoreContainerAliases()\n    {\n        foreach ([\n            'app' => [self::class, \\Illuminate\\Contracts\\Container\\Container::class],\n            'auth' => [\\Illuminate\\Auth\\AuthManager::class],\n        ] as $key => $aliases) {}\n    }\n}\n";
        let read = reader(&[
            ("composer.json", COMPOSER),
            (
                "src/Illuminate/Validation/Concerns/ValidatesAttributes.php",
                attributes,
            ),
            ("src/Illuminate/Validation/Validator.php", validator),
            ("src/Illuminate/View/Compilers/BladeCompiler.php", compiler),
            (
                "src/Illuminate/View/Compilers/Concerns/CompilesConditionals.php",
                conditionals,
            ),
            (
                "src/Illuminate/Database/Eloquent/Concerns/HasAttributes.php",
                attributes_trait,
            ),
            (
                "src/Illuminate/Foundation/Configuration/Middleware.php",
                middleware,
            ),
            ("src/Illuminate/Foundation/Application.php", application),
        ]);
        let fw = load(&read, "/p");
        let keys = |entries: &[Entry]| entries.iter().map(|e| e.key.clone()).collect::<Vec<_>>();
        assert_eq!(
            keys(&fw.rules),
            [
                "accepted",
                "between",
                "current_password",
                "hex_color",
                "required",
                "sometimes"
            ]
        );
        let between = fw.rules.iter().find(|e| e.key == "between").unwrap();
        assert_eq!(between.detail.as_deref(), Some("between:min,max"));
        assert!(between.location.path.ends_with("ValidatesAttributes.php"));
        assert_eq!(between.location.range.start.line, 7);
        assert_eq!(
            keys(&fw.directives),
            ["endauth", "bool", "if", "endphp", "verbatim", "endverbatim"]
        );
        assert_eq!(fw.casts, ["array", "bool", "hashed"]);
        assert!(!fw.casts_method);
        assert_eq!(keys(&fw.middleware), ["auth", "throttle", "web", "api"]);
        assert_eq!(fw.middleware[0].detail.as_deref(), Some("Authenticate"));
        assert_eq!(
            fw.container,
            [
                (
                    "app".to_owned(),
                    "Illuminate\\Foundation\\Application".to_owned()
                ),
                (
                    "auth".to_owned(),
                    "Illuminate\\Auth\\AuthManager".to_owned()
                )
            ]
        );
        let mut fw = fw;
        let stringable = "Illuminate\\Database\\Eloquent\\Casts\\AsStringable";
        fw.check(
            &|path: &str| read(path).is_some(),
            ["Illuminate\\Foundation\\Application", stringable],
            ["livewire/livewire"],
        );
        assert!(fw.has_class("Illuminate\\Foundation\\Application"));
        assert!(!fw.has_class(stringable));
        assert!(fw.has_class("Illuminate\\Never\\Checked"));
        assert!(!fw.has_package("livewire/livewire"));
    }

    #[test]
    fn older_frameworks_list_casts_in_a_switch_and_have_no_casts_method() {
        let attributes = "<?php\nnamespace Illuminate\\Database\\Eloquent\\Concerns;\n\ntrait HasAttributes\n{\n    protected function castAttribute($key, $value)\n    {\n        switch ($this->getCastType($key)) {\n            case 'int':\n            case 'integer':\n                return (int) $value;\n            case 'collection':\n                return new BaseCollection($value);\n        }\n    }\n}\n";
        let fw = load(
            &reader(&[
                (
                    "composer.json",
                    r#"{"autoload": {"psr-4": {"Illuminate\\": "src/Illuminate/"}}}"#,
                ),
                (
                    "src/Illuminate/Database/Eloquent/Concerns/HasAttributes.php",
                    attributes,
                ),
            ]),
            "/p",
        );
        assert_eq!(fw.casts, ["int", "integer", "collection"]);
        assert!(!fw.casts_method && fw.middleware.is_empty() && fw.directives.is_empty());
    }

    #[test]
    fn collects_methods_through_traits_and_parents() {
        let eloquent = "<?php\nnamespace Illuminate\\Database\\Eloquent;\n\nuse Illuminate\\Support\\Collection as BaseCollection;\n\nclass Collection extends BaseCollection\n{\n    public function load($relations) { return $this; }\n    public function find($key, $default = null) {}\n}\n";
        let base = "<?php\nnamespace Illuminate\\Support;\n\nuse Illuminate\\Support\\Traits\\EnumeratesValues;\n\nclass Collection\n{\n    use EnumeratesValues;\n\n    public static function make($items = []) {}\n    public function find($key) {}\n    public function map(callable $callback) {}\n    protected function hidden() {}\n}\n";
        let enumerates = "<?php\nnamespace Illuminate\\Support\\Traits;\n\ntrait EnumeratesValues\n{\n    public function each(callable $callback) {}\n}\n";
        let fw = load(
            &reader(&[
                ("composer.json", COMPOSER),
                ("src/Illuminate/Database/Eloquent/Collection.php", eloquent),
                ("src/Illuminate/Collections/Collection.php", base),
                (
                    "src/Illuminate/Collections/Traits/EnumeratesValues.php",
                    enumerates,
                ),
            ]),
            "/p",
        );
        let names: Vec<&str> = fw.collection.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["load", "find", "map", "each"]);
        assert_eq!(fw.collection[1].1, "($key, $default = null)");
    }

    #[test]
    #[ignore = "needs LARAVEL_FRAMEWORKS pointing at a folder of laravel/framework checkouts"]
    fn reads_every_framework_version() {
        let dir = std::env::var("LARAVEL_FRAMEWORKS").expect("LARAVEL_FRAMEWORKS");
        let mut versions: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        versions.sort();
        for version in versions {
            let read = |path: &str| {
                let rel = path.strip_prefix(BASE)?;
                std::fs::read_to_string(version.join(rel)).ok()
            };
            let fw = load(&read, "/p");
            let name = version.file_name().unwrap().to_string_lossy().into_owned();
            let rules: Vec<&str> = fw.rules.iter().map(|e| e.key.as_str()).collect();
            let directives: Vec<&str> = fw.directives.iter().map(|e| e.key.as_str()).collect();
            let middleware: Vec<&str> = fw.middleware.iter().map(|e| e.key.as_str()).collect();
            println!(
                "== {name}\nfacades {:?}\nrules {}: {rules:?}\ndirectives {}: {directives:?}\ncasts {:?} method={}\nmiddleware {middleware:?}\ncontainer {}\ncollection {} query {} hasMany {} belongsToMany {}",
                fw.facades,
                rules.len(),
                directives.len(),
                fw.casts,
                fw.casts_method,
                fw.container.len(),
                fw.collection.len(),
                fw.query.len(),
                fw.relations["hasMany"].len(),
                fw.relations["belongsToMany"].len(),
            );
            for required in [
                "required",
                "string",
                "max",
                "nullable",
                "sometimes",
                "bail",
                "exists",
                "unique",
            ] {
                assert!(rules.contains(&required), "{name} misses rule {required}");
            }
            for directive in [
                "if",
                "endif",
                "foreach",
                "endforeach",
                "csrf",
                "section",
                "yield",
                "php",
                "endphp",
            ] {
                assert!(
                    directives.contains(&directive),
                    "{name} misses @{directive}"
                );
            }
            assert!(fw.casts.iter().any(|c| c == "datetime"), "{name} casts");
            assert!(
                fw.container.iter().any(|(k, _)| k == "cache"),
                "{name} container"
            );
            for method in ["where", "orderBy", "get", "first", "paginate", "with"] {
                assert!(
                    fw.query.iter().any(|(n, _)| n == method),
                    "{name} query {method}"
                );
            }
            for method in ["map", "filter", "load", "pluck", "first"] {
                assert!(
                    fw.collection.iter().any(|(n, _)| n == method),
                    "{name} collection {method}"
                );
            }
            assert!(
                fw.relations["belongsToMany"]
                    .iter()
                    .any(|(n, _)| n == "sync"),
                "{name} sync"
            );
            assert!(
                fw.relations["hasMany"]
                    .iter()
                    .any(|(n, _)| n == "createMany"),
                "{name} createMany"
            );
        }
    }
}
