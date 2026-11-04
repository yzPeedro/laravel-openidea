use super::Entry;
use crate::php::{CallKind, string_args, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::Project;
use std::collections::BTreeSet;

const COLUMN_TYPES: [&str; 62] = [
    "bigIncrements",
    "bigInteger",
    "binary",
    "boolean",
    "char",
    "dateTimeTz",
    "dateTime",
    "date",
    "decimal",
    "double",
    "enum",
    "float",
    "foreignId",
    "foreignIdFor",
    "foreignUlid",
    "foreignUuid",
    "geography",
    "geometry",
    "increments",
    "integer",
    "ipAddress",
    "json",
    "jsonb",
    "longText",
    "macAddress",
    "mediumIncrements",
    "mediumInteger",
    "mediumText",
    "morphs",
    "nullableMorphs",
    "nullableUlidMorphs",
    "nullableUuidMorphs",
    "set",
    "smallIncrements",
    "smallInteger",
    "softDeletesTz",
    "softDeletes",
    "string",
    "text",
    "timeTz",
    "time",
    "timestampTz",
    "timestamp",
    "tinyIncrements",
    "tinyInteger",
    "tinyText",
    "unsignedBigInteger",
    "unsignedInteger",
    "unsignedMediumInteger",
    "unsignedSmallInteger",
    "unsignedTinyInteger",
    "ulidMorphs",
    "uuidMorphs",
    "ulid",
    "uuid",
    "year",
    "vector",
    "id",
    "rememberToken",
    "timestamps",
    "timestampsTz",
    "nullableTimestamps",
];

#[derive(Debug, PartialEq, Eq)]
pub struct Column {
    pub table: String,
    pub name: String,
    pub kind: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub created: bool,
}

pub fn parse(text: &str) -> (Vec<Table>, Vec<Column>) {
    let args = string_args(&tokenize(text, 0, text.len()));
    let mut tables = Vec::new();
    let mut columns = Vec::new();
    let mut current: Option<String> = None;
    for a in &args {
        let c = &a.callee;
        if c.kind == CallKind::Static
            && c.class.as_deref() == Some("schema")
            && matches!(c.name.as_str(), "create" | "table")
            && a.arg == 0
        {
            current = Some(a.value.clone());
            tables.push(Table {
                name: a.value.clone(),
                start: a.start,
                end: a.end,
                created: c.name == "create",
            });
            continue;
        }
        let Some(table) = &current else { continue };
        if c.kind != CallKind::Method
            || a.arg != 0
            || a.in_array
            || !a.head.as_deref().is_some_and(|h| h.starts_with('$'))
        {
            continue;
        }
        let Some(kind) = COLUMN_TYPES
            .iter()
            .find(|t| t.eq_ignore_ascii_case(&c.name))
        else {
            continue;
        };
        let names: Vec<String> = match *kind {
            "morphs" | "nullableMorphs" | "uuidMorphs" | "ulidMorphs" | "nullableUuidMorphs"
            | "nullableUlidMorphs" => {
                vec![format!("{}_id", a.value), format!("{}_type", a.value)]
            }
            _ => vec![a.value.clone()],
        };
        for name in names {
            columns.push(Column {
                table: table.clone(),
                name,
                kind: (*kind).to_owned(),
                start: a.start,
                end: a.end,
            });
        }
    }
    let tokens = tokenize(text, 0, text.len());
    let mut table_at: Vec<(usize, String)> =
        tables.iter().map(|t| (t.start, t.name.clone())).collect();
    table_at.sort();
    for (i, t) in tokens.iter().enumerate() {
        let crate::php::Kind::Ident(name) = &t.kind else {
            continue;
        };
        let no_args = tokens.get(i + 1).map(|n| &n.kind) == Some(&crate::php::Kind::Open('('))
            && tokens.get(i + 2).map(|n| &n.kind) == Some(&crate::php::Kind::Close(')'));
        let receiver = i >= 2
            && matches!(tokens[i - 1].kind, crate::php::Kind::ObjectOp)
            && matches!(tokens[i - 2].kind, crate::php::Kind::Var(_));
        if !no_args || !receiver {
            continue;
        }
        let implied: &[&str] = match name.as_str() {
            "id" => &["id"],
            "timestamps" | "timestampsTz" | "nullableTimestamps" => &["created_at", "updated_at"],
            "softDeletes" | "softDeletesTz" => &["deleted_at"],
            "rememberToken" => &["remember_token"],
            _ => continue,
        };
        let Some((_, table)) = table_at.iter().rev().find(|(s, _)| *s < t.start) else {
            continue;
        };
        for col in implied {
            columns.push(Column {
                table: table.clone(),
                name: (*col).to_owned(),
                kind: name.clone(),
                start: t.start,
                end: t.end,
            });
        }
    }
    (tables, columns)
}

fn migrations(project: &Project, root: &str) -> Vec<(String, String)> {
    let mut paths = find(project, root, "database/migrations/**/*.php", 10_000);
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| project.read_file(&p).ok().map(|t| (p, t)))
        .collect()
}

pub fn tables(project: &Project, root: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (path, text) in migrations(project, root) {
        let lines = Lines::new(&text);
        for t in parse(&text).0.into_iter().filter(|t| t.created) {
            if seen.insert(t.name.clone()) {
                let detail = path.rsplit(['/', '\\']).next().map(str::to_owned);
                out.push(Entry::new(
                    t.name,
                    detail,
                    &path,
                    lines.range(&text, t.start, t.end),
                ));
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn columns(project: &Project, root: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (path, text) in migrations(project, root) {
        let lines = Lines::new(&text);
        for c in parse(&text).1 {
            let key = format!("{}.{}", c.table, c.name);
            if seen.insert(key.clone()) {
                out.push(Entry::new(
                    key,
                    Some(c.kind),
                    &path,
                    lines.range(&text, c.start, c.end),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tables_and_columns_from_migrations() {
        let text = "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('posts', function (Blueprint $table) {\n            $table->id();\n            $table->foreignId('user_id')->constrained();\n            $table->string('title', 120)->nullable();\n            $table->morphs('commentable');\n            $table->timestamps();\n        });\n        Schema::table('users', function (Blueprint $table) {\n            $table->boolean('is_admin')->default(false);\n        });\n    }\n};\n";
        let (tables, columns) = parse(text);
        assert_eq!(
            tables
                .iter()
                .map(|t| (t.name.as_str(), t.created))
                .collect::<Vec<_>>(),
            [("posts", true), ("users", false)]
        );
        let cols: Vec<_> = columns
            .iter()
            .map(|c| format!("{}.{}", c.table, c.name))
            .collect();
        assert_eq!(
            cols,
            [
                "posts.user_id",
                "posts.title",
                "posts.commentable_id",
                "posts.commentable_type",
                "users.is_admin",
                "posts.id",
                "posts.created_at",
                "posts.updated_at",
            ]
        );
    }
}
