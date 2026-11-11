use crate::project::{Lines, find, read_listed, rel};
use crate::querysql::{Kind, Tok, tokenize};
use crate::sections::{Row, Table, facet};
use openidea_plugin::Project;
use serde::Deserialize;

pub const COLUMNS: [&str; 4] = ["Problem", "Where", "Detail", "Code"];

const GLOBS: [&str; 4] = [
    "app/**/*.php",
    "routes/**/*.php",
    "bootstrap/app.php",
    "resources/views/**/*.blade.php",
];

const WRITE_METHODS: [&str; 4] = ["POST", "PUT", "PATCH", "DELETE"];

fn sym(t: Option<&Tok>, s: &str) -> bool {
    t.is_some_and(|t| t.kind == Kind::Sym && t.text == s)
}

fn line_of(text: &str, at: usize) -> u32 {
    u32::try_from(text[..at.min(text.len())].matches('\n').count() + 1).unwrap_or(1)
}

fn problem(code: &str) -> &'static str {
    match code {
        "laravel.mass-assign-input" => "Every input field mass assigned",
        "laravel.raw-sql-variable" => "Variable in raw SQL",
        "laravel.unescaped-request" => "Request data printed unescaped",
        _ => "Security",
    }
}

fn strings_until_close(toks: &[Tok], open: usize) -> Vec<&Tok> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    for t in &toks[open..] {
        match (t.kind == Kind::Sym, t.text.as_str()) {
            (true, "(" | "[" | "{") => depth += 1,
            (true, ")" | "]" | "}") => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ if t.kind == Kind::Str => out.push(t),
            _ => {}
        }
    }
    out
}

pub fn file_rows(rel_path: &str, path: &str, text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let found = if rel_path.ends_with(".blade.php") {
        crate::security::blade(text, path)
    } else {
        crate::security::php(text, path)
    };
    let lines = Lines::new(text);
    for f in found {
        let line = lines.position(text, f.start).line + 1;
        rows.push(Row::at(
            vec![
                problem(f.code).into(),
                format!("{rel_path}:{line}"),
                f.message,
                f.code.into(),
            ],
            rel_path,
            line,
        ));
    }
    if rel_path.ends_with(".blade.php") {
        return rows;
    }
    let toks = tokenize(text);
    for (i, t) in toks.iter().enumerate() {
        if t.kind == Kind::Var
            && t.text == "guarded"
            && sym(toks.get(i + 1), "=")
            && sym(toks.get(i + 2), "[")
            && sym(toks.get(i + 3), "]")
        {
            let line = line_of(text, t.start);
            rows.push(Row::at(
                vec![
                    "Mass assignment open".into(),
                    format!("{rel_path}:{line}"),
                    "$guarded = [] lets create() and update() fill every column, including ones like is_admin".into(),
                    "laravel.guarded-empty".into(),
                ],
                rel_path,
                line,
            ));
        }
        if t.kind == Kind::Ident
            && t.text == "unguard"
            && sym(toks.get(i.wrapping_sub(1)), "::")
            && sym(toks.get(i + 1), "(")
        {
            let line = line_of(text, t.start);
            rows.push(Row::at(
                vec![
                    "Mass assignment open".into(),
                    format!("{rel_path}:{line}"),
                    "Model::unguard() turns off $fillable and $guarded for every model".into(),
                    "laravel.unguard".into(),
                ],
                rel_path,
                line,
            ));
        }
        let csrf_call =
            t.kind == Kind::Ident && t.text == "validateCsrfTokens" && sym(toks.get(i + 1), "(");
        let csrf_property = t.kind == Kind::Var
            && t.text == "except"
            && rel_path.contains("VerifyCsrfToken")
            && sym(toks.get(i + 1), "=");
        if csrf_call || csrf_property {
            let open = if csrf_call { i + 1 } else { i + 2 };
            for s in strings_until_close(&toks, open) {
                let line = line_of(text, s.start);
                rows.push(Row::at(
                    vec![
                        "CSRF check skipped".into(),
                        format!("{rel_path}:{line}"),
                        format!(
                            "'{}' accepts POST, PUT, PATCH and DELETE without a CSRF token",
                            s.text
                        ),
                        "laravel.csrf-except".into(),
                    ],
                    rel_path,
                    line,
                ));
            }
        }
    }
    rows
}

pub fn section(project: &Project, root: &str) -> Table {
    let mut rows = Vec::new();
    let mut paths: Vec<String> = GLOBS
        .iter()
        .flat_map(|g| find(project, root, g, 20_000))
        .collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        let rel_path = rel(root, &path).replace('\\', "/");
        let Ok(text) = read_listed(project, &path) else {
            continue;
        };
        rows.extend(file_rows(&rel_path, &path, &text));
    }
    Table {
        columns: COLUMNS.to_vec(),
        rows,
        facets: vec![facet("Problem")],
    }
}

#[derive(Deserialize)]
struct ListedRoute {
    #[serde(default)]
    method: String,
    #[serde(default)]
    uri: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    action: String,
    #[serde(default)]
    middleware: serde_json::Value,
}

fn guarded(middleware: &[String]) -> bool {
    middleware.iter().any(|m| {
        let m = m.trim();
        let short = m.rsplit('\\').next().unwrap_or(m);
        m.starts_with("auth")
            || m.starts_with("can:")
            || m.starts_with("signed")
            || m.starts_with("guest")
            || m.starts_with("ability")
            || m.starts_with("abilities")
            || m.starts_with("role")
            || m.starts_with("permission")
            || short.starts_with("Authenticate")
            || short.starts_with("Authorize")
            || short.starts_with("ValidateSignature")
            || short.starts_with("RedirectIfAuthenticated")
    })
}

pub fn open_routes(stdout: &str) -> Result<Vec<Row>, String> {
    let start = stdout.find('[').ok_or_else(|| {
        "route:list --json returned no JSON (it needs Laravel 9 or newer)".to_owned()
    })?;
    let end = stdout.rfind(']').map_or(stdout.len(), |e| e + 1);
    let routes: Vec<ListedRoute> = serde_json::from_str(&stdout[start..end])
        .map_err(|e| format!("route:list --json returned something else: {e}"))?;
    let mut rows = Vec::new();
    for r in routes {
        let methods: Vec<&str> = r.method.split('|').collect();
        let writes: Vec<&str> = methods
            .iter()
            .copied()
            .filter(|m| WRITE_METHODS.contains(m))
            .collect();
        if writes.is_empty() {
            continue;
        }
        let middleware: Vec<String> = match &r.middleware {
            serde_json::Value::Array(items) => items
                .iter()
                .filter_map(|m| m.as_str().map(str::to_owned))
                .collect(),
            serde_json::Value::String(s) => s.split([',', '\n']).map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        let package = r.name.as_deref().is_some_and(|n| {
            crate::unused::PACKAGE_ROUTE_PREFIXES
                .iter()
                .any(|p| n.starts_with(p))
        });
        if package || guarded(&middleware) {
            continue;
        }
        let shown = middleware
            .iter()
            .map(|m| m.trim())
            .filter(|m| !m.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(Row::at(
            vec![
                "Write route without auth".into(),
                format!("{} /{}", writes.join("|"), r.uri.trim_start_matches('/')),
                format!(
                    "{}{} · middleware: {}",
                    r.name.map(|n| format!("{n} · ")).unwrap_or_default(),
                    r.action,
                    if shown.is_empty() { "none" } else { &shown }
                ),
                "laravel.route-without-auth".into(),
            ],
            "",
            0,
        ));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_open_mass_assignment_and_csrf_exceptions() {
        let model = "<?php\nclass Post extends Model\n{\n    protected $guarded = [];\n}\n";
        let provider = "<?php\nclass AppServiceProvider\n{\n    public function boot(): void\n    {\n        Model::unguard();\n    }\n}\n";
        let app = "<?php\nreturn Application::configure(basePath: dirname(__DIR__))\n    ->withMiddleware(function (Middleware $middleware) {\n        $middleware->validateCsrfTokens(except: [\n            'stripe/*',\n            'webhooks/github',\n        ]);\n    })->create();\n";
        let old = "<?php\nclass VerifyCsrfToken extends Middleware\n{\n    protected $except = [\n        'paypal/ipn',\n    ];\n}\n";
        let cell = |rows: Vec<Row>| -> Vec<String> {
            rows.into_iter()
                .map(|r| format!("{} @ {}", r.cells[0], r.cells[1]))
                .collect()
        };
        assert_eq!(
            cell(file_rows(
                "app/Models/Post.php",
                "/p/app/Models/Post.php",
                model
            )),
            ["Mass assignment open @ app/Models/Post.php:4"]
        );
        assert_eq!(
            cell(file_rows(
                "app/Providers/AppServiceProvider.php",
                "/p/a.php",
                provider
            )),
            ["Mass assignment open @ app/Providers/AppServiceProvider.php:6"]
        );
        assert_eq!(
            cell(file_rows("bootstrap/app.php", "/p/bootstrap/app.php", app)),
            [
                "CSRF check skipped @ bootstrap/app.php:5",
                "CSRF check skipped @ bootstrap/app.php:6"
            ]
        );
        let rows = file_rows("app/Http/Middleware/VerifyCsrfToken.php", "/p/v.php", old);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].cells[2].starts_with("'paypal/ipn'"));
        assert_eq!(rows[0].open.as_ref().map(|t| t.line), Some(5));
    }

    #[test]
    fn finds_write_routes_without_auth_middleware() {
        let out = "Warning: something\n[{\"domain\":null,\"method\":\"POST\",\"uri\":\"orders\",\"name\":\"orders.store\",\"action\":\"App\\\\Http\\\\Controllers\\\\OrderController@store\",\"middleware\":[\"web\"]},{\"domain\":null,\"method\":\"GET|HEAD\",\"uri\":\"orders\",\"name\":null,\"action\":\"Closure\",\"middleware\":[\"web\"]},{\"domain\":null,\"method\":\"PUT|PATCH\",\"uri\":\"orders/{order}\",\"name\":null,\"action\":\"X@update\",\"middleware\":[\"web\",\"auth\"]},{\"domain\":null,\"method\":\"POST\",\"uri\":\"login\",\"name\":\"login\",\"action\":\"X@login\",\"middleware\":[\"web\",\"guest\"]},{\"domain\":null,\"method\":\"PUT\",\"uri\":\"storage/{path}\",\"name\":\"storage.local.upload\",\"action\":\"Closure\",\"middleware\":[]},{\"domain\":null,\"method\":\"DELETE\",\"uri\":\"api/tokens\",\"name\":null,\"action\":\"X@destroy\",\"middleware\":[\"api\",\"Illuminate\\\\Auth\\\\Middleware\\\\Authenticate:sanctum\"]}]\n";
        let rows = open_routes(out).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cells[1], "POST /orders");
        assert_eq!(
            rows[0].cells[2],
            "orders.store · App\\Http\\Controllers\\OrderController@store · middleware: web"
        );
        assert!(open_routes("ERROR  The command isn't defined").is_err());
    }
}
