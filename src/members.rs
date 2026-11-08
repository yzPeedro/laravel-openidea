use crate::index::{Entry, plural};
use crate::php::{Kind, tokenize};
use openidea_plugin::providers::{CompletionItem, CompletionKind};
use openidea_plugin::{Position, Range};

const SKIP_FILLABLE: [&str; 5] = [
    "id",
    "created_at",
    "updated_at",
    "deleted_at",
    "remember_token",
];
const SENSITIVE: [&str; 7] = [
    "password",
    "remember_token",
    "api_token",
    "token",
    "secret",
    "two_factor_secret",
    "two_factor_recovery_codes",
];
const RELATION_FQN: &str = "\\Illuminate\\Database\\Eloquent\\Relations";

pub struct Model<'a> {
    pub table: &'a str,
    pub columns: Vec<(String, String)>,
    pub models: Vec<(String, String)>,
    pub foreign: Vec<(String, String)>,
    pub relations: Vec<String>,
    pub casts_method: bool,
}

pub struct Context {
    pub start: usize,
    pub typed: String,
    pub indent: String,
}

pub fn context(text: &str, cursor: usize) -> Option<Context> {
    let line_start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..cursor];
    let word_start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .map_or(0, |i| i + 1);
    let prefix = &before[..word_start];
    if !prefix.split_whitespace().all(|w| {
        matches!(
            w,
            "public" | "protected" | "private" | "static" | "function"
        )
    }) {
        return None;
    }
    let depth = tokenize(text, 0, line_start)
        .iter()
        .fold(0i32, |d, t| match t.kind {
            Kind::Open('{') => d + 1,
            Kind::Close('}') => d - 1,
            _ => d,
        });
    if depth != 1 {
        return None;
    }
    let indent: String = before.chars().take_while(|c| c.is_whitespace()).collect();
    Some(Context {
        start: line_start + indent.len(),
        typed: before[word_start..].trim_start_matches('$').to_owned(),
        indent,
    })
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn lcfirst(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_lowercase().chain(c).collect())
}

pub fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("ies") {
        format!("{stem}y")
    } else if ["ses", "xes", "zes", "ches", "shes"]
        .iter()
        .any(|s| word.ends_with(s))
    {
        word[..word.len() - 2].to_owned()
    } else {
        word.strip_suffix('s').unwrap_or(word).to_owned()
    }
}

fn cast_for(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "date" => "date",
        "dateTime" | "dateTimeTz" | "timestamp" | "timestampTz" => "datetime",
        "boolean" => "boolean",
        "json" | "jsonb" => "array",
        "decimal" => "decimal:2",
        "float" | "double" => "float",
        "integer" | "bigInteger" | "smallInteger" | "tinyInteger" | "mediumInteger"
        | "unsignedInteger" | "unsignedBigInteger" => "integer",
        _ => return None,
    })
}

fn list(indent: &str, values: &[String]) -> String {
    if values.is_empty() {
        return "[]".into();
    }
    let rows: Vec<String> = values
        .iter()
        .map(|v| format!("{indent}    '{v}',"))
        .collect();
    format!("[\n{}\n{indent}]", rows.join("\n"))
}

fn method(indent: &str, signature: &str, body: &str) -> String {
    format!("{signature}\n{indent}{{\n{indent}    {body}\n{indent}}}")
}

pub fn items(m: &Model, ctx: &Context, at: Position, start: Position) -> Vec<CompletionItem> {
    let i = &ctx.indent;
    let names: Vec<String> = m.columns.iter().map(|(c, _)| c.clone()).collect();
    let fillable: Vec<String> = names
        .iter()
        .filter(|c| !SKIP_FILLABLE.contains(&c.as_str()))
        .cloned()
        .collect();
    let hidden: Vec<String> = names
        .iter()
        .filter(|c| SENSITIVE.contains(&c.as_str()))
        .cloned()
        .collect();
    let casts: Vec<(&String, &str)> = m
        .columns
        .iter()
        .filter(|(c, _)| !matches!(c.as_str(), "created_at" | "updated_at"))
        .filter_map(|(c, k)| Some((c, cast_for(k)?)))
        .collect();
    let cast_rows = |pad: &str| {
        casts
            .iter()
            .map(|(c, cast)| format!("{pad}'{c}' => '{cast}',"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let casts_member = if m.casts_method {
        let body = if casts.is_empty() {
            "return [];".to_owned()
        } else {
            format!(
                "return [\n{}\n{i}    ];",
                cast_rows(&format!("{i}        "))
            )
        };
        (
            "casts()".to_owned(),
            format!("{} casts from the migrations", casts.len()),
            method(i, "protected function casts(): array", &body),
        )
    } else {
        let list = if casts.is_empty() {
            "[]".to_owned()
        } else {
            format!("[\n{}\n{i}]", cast_rows(&format!("{i}    ")))
        };
        (
            "$casts".to_owned(),
            format!("{} casts from the migrations", casts.len()),
            format!("protected $casts = {list};"),
        )
    };
    let mut out: Vec<(String, String, String)> = vec![
        (
            "$fillable".into(),
            format!("{} columns", fillable.len()),
            format!("protected $fillable = {};", list(i, &fillable)),
        ),
        (
            "$guarded".into(),
            "only id".into(),
            "protected $guarded = ['id'];".into(),
        ),
        (
            "$hidden".into(),
            "sensitive columns".into(),
            format!("protected $hidden = {};", list(i, &hidden)),
        ),
        casts_member,
        (
            "$table".into(),
            m.table.to_owned(),
            format!("protected $table = '{}';", m.table),
        ),
        (
            "$primaryKey".into(),
            "key column".into(),
            "protected $primaryKey = 'id';".into(),
        ),
        (
            "$keyType".into(),
            "string keys".into(),
            "protected $keyType = 'string';".into(),
        ),
        (
            "$incrementing".into(),
            "non-incrementing keys".into(),
            "public $incrementing = false;".into(),
        ),
        (
            "$timestamps".into(),
            "no timestamps".into(),
            "public $timestamps = false;".into(),
        ),
        (
            "$connection".into(),
            "database connection".into(),
            "protected $connection = 'mysql';".into(),
        ),
        (
            "$with".into(),
            "eager-loaded relations".into(),
            "protected $with = [];".into(),
        ),
        (
            "$withCount".into(),
            "counted relations".into(),
            "protected $withCount = [];".into(),
        ),
        (
            "$appends".into(),
            "appended accessors".into(),
            "protected $appends = [];".into(),
        ),
        (
            "$touches".into(),
            "touched relations".into(),
            "protected $touches = [];".into(),
        ),
        (
            "$perPage".into(),
            "pagination size".into(),
            "protected $perPage = 15;".into(),
        ),
        (
            "$attributes".into(),
            "default values".into(),
            "protected $attributes = [];".into(),
        ),
        (
            "booted()".into(),
            "model events".into(),
            method(i, "protected static function booted(): void", ""),
        ),
    ];
    for (column, _) in &m.columns {
        let Some(base) = column.strip_suffix("_id") else {
            continue;
        };
        let table = plural(base);
        let Some((model, _)) = m.models.iter().find(|(_, t)| *t == table) else {
            continue;
        };
        let name = camel(base);
        if m.relations.contains(&name) {
            continue;
        }
        out.push((
            format!("{name}()"),
            format!("belongsTo({model}) via {column}"),
            method(
                i,
                &format!("public function {name}(): {RELATION_FQN}\\BelongsTo"),
                &format!("return $this->belongsTo({model}::class);"),
            ),
        ));
    }
    let key = format!("{}_id", singular(m.table));
    for (table, column) in &m.foreign {
        if *column != key || table == m.table {
            continue;
        }
        let Some((model, _)) = m.models.iter().find(|(_, t)| t == table) else {
            continue;
        };
        let name = lcfirst(&plural(model));
        if m.relations.contains(&name) {
            continue;
        }
        out.push((
            format!("{name}()"),
            format!("hasMany({model}) via {table}.{column}"),
            method(
                i,
                &format!("public function {name}(): {RELATION_FQN}\\HasMany"),
                &format!("return $this->hasMany({model}::class);"),
            ),
        ));
    }
    let typed = ctx.typed.to_lowercase();
    out.into_iter()
        .enumerate()
        .filter(|(_, (label, ..))| {
            label
                .trim_start_matches('$')
                .to_lowercase()
                .contains(&typed)
        })
        .map(|(n, (label, detail, insert))| CompletionItem {
            detail: Some(detail),
            insert_text: insert,
            replace: Some(Range::new(start, at)),
            sort_text: Some(format!("{n:03}")),
            ..CompletionItem::new(&label, CompletionKind::Snippet)
        })
        .collect()
}

pub fn columns_of(entries: &[Entry], table: &str) -> Vec<(String, String)> {
    let prefix = format!("{table}.");
    entries
        .iter()
        .filter_map(|e| {
            Some((
                e.key.strip_prefix(&prefix)?.to_owned(),
                e.detail.clone().unwrap_or_default(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_class_body_positions() {
        let text = "<?php\nclass User extends Model\n{\n    protected $fil\n    public function x()\n    {\n        $fil\n    }\n}\n";
        let at = text.find("$fil\n").unwrap() + 4;
        let ctx = context(text, at).unwrap();
        assert_eq!((ctx.typed.as_str(), ctx.indent.as_str()), ("fil", "    "));
        assert_eq!(&text[ctx.start..at], "protected $fil");
        let inside = text.rfind("$fil").unwrap() + 4;
        assert!(context(text, inside).is_none());
        assert_eq!(singular("user_likes"), "user_like");
        assert_eq!(singular("categories"), "category");
    }
}
