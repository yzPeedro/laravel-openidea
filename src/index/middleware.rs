use super::Entry;
use crate::php::{CallKind, string_args, tokenize};
use crate::project::{Lines, join};
use openidea_plugin::{Project, Range};
use std::collections::BTreeSet;

const BUILTIN: [(&str, &str); 14] = [
    ("auth", "Authenticate (auth:guard)"),
    ("auth.basic", "AuthenticateWithBasicAuth"),
    ("auth.session", "AuthenticateSession"),
    ("cache.headers", "SetCacheHeaders"),
    ("can", "Authorize (can:ability,model)"),
    ("guest", "RedirectIfAuthenticated"),
    ("password.confirm", "RequirePassword"),
    ("precognitive", "HandlePrecognitiveRequests"),
    ("signed", "ValidateSignature"),
    ("throttle", "ThrottleRequests (throttle:limiter)"),
    ("verified", "EnsureEmailIsVerified"),
    ("web", "middleware group"),
    ("api", "middleware group"),
    ("bindings", "SubstituteBindings"),
];

const ALIAS_PROPERTIES: [&str; 3] = [
    "$middlewarealiases",
    "$routemiddleware",
    "$middlewaregroups",
];

pub fn parse(path: &str, text: &str) -> Vec<Entry> {
    let lines = Lines::new(text);
    string_args(&tokenize(text, 0, text.len()))
        .into_iter()
        .filter(|a| {
            let c = &a.callee;
            match c.kind {
                CallKind::Method => {
                    a.is_key
                        && matches!(
                            c.name.as_str(),
                            "alias" | "group" | "appendtogroup" | "prependtogroup"
                        )
                        || !a.in_array
                            && a.arg == 0
                            && matches!(
                                c.name.as_str(),
                                "group" | "appendtogroup" | "prependtogroup"
                            )
                }
                CallKind::Property => {
                    a.is_key
                        && ALIAS_PROPERTIES.contains(&format!("${}", c.name).as_str())
                        && !a.in_array
                }
                _ => false,
            }
        })
        .map(|a| {
            let file = path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned();
            Entry::new(a.value, Some(file), path, lines.range(text, a.start, a.end))
        })
        .collect()
}

pub fn index(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for file in ["bootstrap/app.php", "app/Http/Kernel.php"] {
        if let Ok(text) = project.read_file(file) {
            out.extend(parse(&join(root, file), &text));
        }
    }
    let mut seen: BTreeSet<String> = out.iter().map(|e| e.key.clone()).collect();
    for (name, detail) in BUILTIN {
        if seen.insert(name.to_owned()) {
            out.push(Entry::new(
                name,
                Some(detail.to_owned()),
                "",
                Range::default(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_aliases_from_bootstrap_and_kernel() {
        let app = "<?php\nreturn Application::configure()\n    ->withMiddleware(function (Middleware $middleware) {\n        $middleware->alias([\n            'user-is-blocked' => EnsureNotBlocked::class,\n        ]);\n        $middleware->appendToGroup('admin', [Foo::class]);\n    })->create();\n";
        let keys: Vec<_> = parse("/p/bootstrap/app.php", app)
            .into_iter()
            .map(|e| e.key)
            .collect();
        assert_eq!(keys, ["user-is-blocked", "admin"]);
        let kernel = "<?php\nclass Kernel {\n    protected $middlewareAliases = [\n        'role' => Role::class,\n    ];\n}\n";
        let keys: Vec<_> = parse("/p/app/Http/Kernel.php", kernel)
            .into_iter()
            .map(|e| e.key)
            .collect();
        assert_eq!(keys, ["role"]);
    }
}
