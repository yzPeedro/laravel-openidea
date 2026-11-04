use crate::php::{Kind, tokenize};

pub fn column_type(kind: &str) -> &'static str {
    let k = kind.to_ascii_lowercase();
    if k.contains("bool") {
        "bool"
    } else if k.contains("int")
        || k == "id"
        || k.contains("increments")
        || k.starts_with("foreignid")
        || k == "year"
    {
        "int"
    } else if k.contains("decimal") || k.contains("float") || k.contains("double") {
        "float"
    } else if k.contains("json") {
        "array"
    } else if k.contains("date") || k.contains("time") || k.contains("softdelete") {
        "\\Illuminate\\Support\\Carbon|null"
    } else {
        "string"
    }
}

pub fn cast_type(cast: &str) -> Option<String> {
    let c = cast.trim().trim_matches(['\'', '"']);
    let base = c.split(':').next().unwrap_or(c).to_ascii_lowercase();
    let t = match base.as_str() {
        "bool" | "boolean" => "bool",
        "int" | "integer" | "timestamp" => "int",
        "float" | "double" | "real" => "float",
        "decimal" | "string" | "encrypted" | "hashed" => "string",
        "array" | "json" | "encrypted:array" => "array",
        "object" | "encrypted:object" => "object",
        "collection" | "encrypted:collection" | "asCollection" => {
            "\\Illuminate\\Support\\Collection"
        }
        "date" | "datetime" | "immutable_date" | "immutable_datetime" | "custom_datetime" => {
            if base.starts_with("immutable") {
                "\\Carbon\\CarbonImmutable|null"
            } else {
                "\\Illuminate\\Support\\Carbon|null"
            }
        }
        _ => {
            if let Some(class) = c.strip_suffix("::class") {
                return Some(
                    class
                        .trim_start_matches('\\')
                        .rsplit('\\')
                        .next()
                        .unwrap_or(class)
                        .to_owned(),
                );
            }
            return None;
        }
    };
    Some(t.to_owned())
}

pub fn casts(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut bodies = Vec::new();
    if let Some(at) = text.find("$casts") {
        bodies.push(&text[at..]);
    }
    if let Some(at) = text.find("function casts") {
        bodies.push(&text[at..]);
    }
    for body in bodies {
        let Some(open) = body.find('[') else {
            continue;
        };
        let tokens = tokenize(body, open, body.len().min(open + 6000));
        let mut depth = 0i32;
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i].kind {
                Kind::Open('[') => depth += 1,
                Kind::Close(']') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Kind::Str { value, .. }
                    if depth == 1
                        && tokens
                            .get(i + 1)
                            .is_some_and(|t| t.kind == Kind::DoubleArrow) =>
                {
                    let start = tokens.get(i + 2).map_or(0, |t| t.start);
                    let mut end = start;
                    let mut j = i + 2;
                    let mut d = 0i32;
                    while j < tokens.len() {
                        match tokens[j].kind {
                            Kind::Open(_) => d += 1,
                            Kind::Close(_) if d == 0 => break,
                            Kind::Close(_) => d -= 1,
                            Kind::Comma if d == 0 => break,
                            _ => {}
                        }
                        end = tokens[j].end;
                        j += 1;
                    }
                    out.push((value.clone(), body[start..end].trim().to_owned()));
                    i = j;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }
    out
}

pub fn relation_type(detail: &str) -> String {
    let kind = detail.split('(').next().unwrap_or(detail);
    let target = detail
        .split_once('(')
        .map(|(_, t)| {
            t.trim_end_matches(')')
                .trim()
                .rsplit('\\')
                .next()
                .unwrap_or("")
                .to_owned()
        })
        .filter(|t| !t.is_empty());
    let model = target.unwrap_or_else(|| "\\Illuminate\\Database\\Eloquent\\Model".into());
    match kind {
        "hasMany" | "belongsToMany" | "morphMany" | "morphToMany" | "morphedByMany"
        | "hasManyThrough" => {
            format!("\\Illuminate\\Database\\Eloquent\\Collection<int, {model}>")
        }
        "morphTo" => "\\Illuminate\\Database\\Eloquent\\Model|null".into(),
        _ => format!("{model}|null"),
    }
}

pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub fn docblock(
    text: &str,
    class_start: usize,
    columns: &[(String, String)],
    relations: &[(String, String)],
) -> Option<Edit> {
    let casts: Vec<(String, String)> = casts(text);
    let mut lines: Vec<String> = Vec::new();
    for (name, kind) in columns {
        let ty = casts
            .iter()
            .find(|(c, _)| c == name)
            .and_then(|(_, cast)| cast_type(cast))
            .unwrap_or_else(|| column_type(kind).to_owned());
        lines.push(format!(" * @property {ty} ${name}"));
    }
    for (name, detail) in relations {
        lines.push(format!(
            " * @property-read {} ${name}",
            relation_type(detail)
        ));
    }
    if lines.is_empty() {
        return None;
    }
    let line_start = |at: usize| text[..at].rfind('\n').map_or(0, |i| i + 1);
    let mut insert = line_start(class_start);
    loop {
        let prev_end = insert.checked_sub(1)?;
        let prev_start = line_start(prev_end);
        let prev = text[prev_start..prev_end].trim();
        if prev.starts_with("#[")
            || prev.ends_with(")]")
            || prev.ends_with(']') && prev.starts_with('#')
        {
            insert = prev_start;
            continue;
        }
        break;
    }
    let before = text[..insert].trim_end();
    if before.ends_with("*/")
        && let Some(open) = before.rfind("/**")
    {
        let existing = &before[open..];
        let kept: Vec<&str> = existing
            .lines()
            .map(str::trim)
            .filter(|l| {
                !l.starts_with("/**")
                    && !l.starts_with("*/")
                    && !l.contains("@property")
                    && *l != "*"
            })
            .collect();
        let mut body: Vec<String> = kept
            .iter()
            .map(|l| format!(" {}", l.trim_start_matches(['/'])))
            .collect();
        if !body.is_empty() {
            body.push(" *".into());
        }
        body.extend(lines);
        return Some(Edit {
            start: open,
            end: before.len(),
            text: format!("/**\n{}\n */", body.join("\n")),
        });
    }
    Some(Edit {
        start: insert,
        end: insert,
        text: format!("/**\n{}\n */\n", lines.join("\n")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols() -> Vec<(String, String)> {
        vec![
            ("id".into(), "id".into()),
            ("name".into(), "string".into()),
            ("is_admin".into(), "boolean".into()),
            ("status".into(), "string".into()),
            ("meta".into(), "json".into()),
            ("created_at".into(), "timestamps".into()),
        ]
    }

    #[test]
    fn inserts_a_docblock_above_attributes() {
        let text = "<?php\nnamespace App\\Models;\n\n#[ObservedBy([UserObserver::class])]\nclass User extends Model\n{\n    protected function casts(): array\n    {\n        return ['status' => Status::class, 'meta' => 'array'];\n    }\n}\n";
        let at = text.find("class User").unwrap();
        let rels = vec![
            ("posts".to_owned(), "hasMany(Post)".to_owned()),
            ("team".to_owned(), "belongsTo(App\\Models\\Team)".to_owned()),
        ];
        let e = docblock(text, at, &cols(), &rels).unwrap();
        assert_eq!(e.start, text.find("#[Observed").unwrap());
        assert_eq!(e.start, e.end);
        assert_eq!(
            e.text,
            "/**\n * @property int $id\n * @property string $name\n * @property bool $is_admin\n * @property Status $status\n * @property array $meta\n * @property \\Illuminate\\Support\\Carbon|null $created_at\n * @property-read \\Illuminate\\Database\\Eloquent\\Collection<int, Post> $posts\n * @property-read Team|null $team\n */\n"
        );
    }

    #[test]
    fn replaces_old_properties_and_keeps_the_rest() {
        let text = "<?php\n/**\n * User of the app.\n *\n * @property int $old\n */\nclass User extends Model {}\n";
        let at = text.find("class User").unwrap();
        let e = docblock(text, at, &cols()[..2], &[]).unwrap();
        assert_eq!(
            &text[e.start..e.end],
            "/**\n * User of the app.\n *\n * @property int $old\n */"
        );
        assert_eq!(
            e.text,
            "/**\n * User of the app.\n *\n * @property int $id\n * @property string $name\n */"
        );
    }
}
