use super::Entry;
use crate::php::{CallKind, Callee, string_args, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::Project;
use std::collections::BTreeSet;

pub const IN: &[(&str, &str)] = &[
    ("path", "path parameter"),
    ("query", "query string"),
    ("header", "request header"),
    ("cookie", "cookie"),
];

pub const TYPES: &[(&str, &str)] = &[
    ("string", ""),
    ("integer", ""),
    ("number", ""),
    ("boolean", ""),
    ("array", "needs items"),
    ("object", "needs properties"),
    ("null", "OpenAPI 3.1"),
];

pub const FORMATS: &[(&str, &str)] = &[
    ("int32", "integer"),
    ("int64", "integer"),
    ("float", "number"),
    ("double", "number"),
    ("byte", "base64 string"),
    ("binary", "file contents"),
    ("date", "full-date"),
    ("date-time", "RFC 3339"),
    ("time", "RFC 3339 time"),
    ("duration", "ISO 8601"),
    ("password", "masked string"),
    ("email", "email address"),
    ("idn-email", ""),
    ("hostname", ""),
    ("ipv4", ""),
    ("ipv6", ""),
    ("uri", ""),
    ("uri-reference", ""),
    ("url", ""),
    ("uuid", ""),
    ("regex", ""),
];

pub fn is_openapi(c: &Callee) -> bool {
    matches!(c.kind, CallKind::Function | CallKind::Attribute)
        && c.namespace.as_deref().is_some_and(|n| {
            n == "oa" || n.ends_with("openapi\\attributes") || n.ends_with("openapi\\annotations")
        })
}

pub fn section_of(callee: &str, param: &str) -> Option<&'static str> {
    Some(match (callee, param) {
        ("schema", "schema") => "schemas",
        ("response", "response") => "responses",
        ("parameter", "parameter") => "parameters",
        ("requestbody", "request") => "requestBodies",
        ("header", "header") => "headers",
        ("securityscheme", "securityScheme") => "securitySchemes",
        ("examples", "example") => "examples",
        ("tag", "name") => "tags",
        _ => return None,
    })
}

pub fn parse(path: &str, text: &str) -> Vec<Entry> {
    let lines = Lines::new(text);
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned();
    string_args(&tokenize(text, 0, text.len()))
        .into_iter()
        .filter(|a| is_openapi(&a.callee) && !a.is_key)
        .filter_map(|a| {
            let param = a.param.as_deref()?;
            let section = match section_of(&a.callee.name, param) {
                Some("responses")
                    if a.value.chars().all(|c| c.is_ascii_digit()) || a.value == "default" =>
                {
                    return None;
                }
                Some(s) => s,
                None if param == "tags" && a.in_array => "tags",
                None => return None,
            };
            let detail = if section == "tags" {
                None
            } else {
                Some(file.clone())
            };
            Some(Entry::new(
                format!("{section}/{}", a.value),
                detail,
                path,
                lines.range(text, a.start, a.end),
            ))
        })
        .collect()
}

pub fn index(project: &Project, root: &str) -> Vec<Entry> {
    let mut paths = find(project, root, "app/**/*.php", 20_000);
    paths.extend(find(project, root, "docs/**/*.php", 2000));
    paths.extend(find(project, root, "routes/**/*.php", 500));
    let mut seen = BTreeSet::new();
    let mut definitions = Vec::new();
    let mut tags = Vec::new();
    for path in paths {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        if !text.contains("OA\\") && !text.contains("OpenApi\\") {
            continue;
        }
        for entry in parse(&path, &text) {
            if entry.detail.is_some() {
                definitions.push(entry);
            } else {
                tags.push(entry);
            }
        }
    }
    definitions
        .into_iter()
        .chain(tags)
        .filter(|e| seen.insert(e.key.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_components_and_tags() {
        let text = "<?php\nuse OpenApi\\Attributes as OA;\n\n#[OA\\Schema(\n    schema: 'UserNotFoundSchema',\n    required: ['message'],\n    properties: [new OA\\Property(property: 'message', type: 'string')],\n)]\nclass UserNotFound {}\n\n#[OA\\SecurityScheme(securityScheme: 'bearerAuth', type: 'http')]\n#[OA\\Post(path: '/users', tags: ['Users'], responses: [new OA\\Response(response: 204, description: 'ok'), new OA\\Response(response: 'NotFound', description: 'x')])]\nclass C {}\n";
        let keys: Vec<_> = parse("/p/app/Docs.php", text)
            .into_iter()
            .map(|e| e.key)
            .collect();
        assert_eq!(
            keys,
            [
                "schemas/UserNotFoundSchema",
                "securitySchemes/bearerAuth",
                "tags/Users",
                "responses/NotFound"
            ]
        );
    }
}
