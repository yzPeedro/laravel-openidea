use super::Entry;
use crate::php::{CallKind, Kind, Token, string_args, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::{Project, Range};

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

const INCREMENTS: [&str; 6] = [
    "id",
    "increments",
    "bigIncrements",
    "mediumIncrements",
    "smallIncrements",
    "tinyIncrements",
];

const MORPHS: [&str; 6] = [
    "morphs",
    "nullableMorphs",
    "uuidMorphs",
    "ulidMorphs",
    "nullableUuidMorphs",
    "nullableUlidMorphs",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub table: String,
    pub name: String,
    pub kind: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub table: String,
    pub columns: Vec<String>,
    pub kind: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foreign {
    pub table: String,
    pub column: String,
    pub on: String,
    pub references: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Table(Table),
    Column(Column),
    DropColumn {
        table: String,
        name: String,
    },
    RenameColumn {
        table: String,
        from: String,
        to: String,
    },
    DropTable(String),
    RenameTable {
        from: String,
        to: String,
    },
    Index(Index),
    Foreign(Foreign),
}

struct Call {
    name: String,
    schema: bool,
    open: usize,
    args: Vec<(usize, String, usize, usize)>,
}

fn matching_close(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (k, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    tokens.len().saturating_sub(1)
}

fn down_body(tokens: &[Token]) -> Option<(usize, usize)> {
    let at = tokens.windows(2).position(|w| {
        matches!(&w[0].kind, Kind::Ident(f) if f.eq_ignore_ascii_case("function"))
            && matches!(&w[1].kind, Kind::Ident(n) if n == "down")
    })?;
    let open = (at..tokens.len()).find(|&k| tokens[k].kind == Kind::Open('{'))?;
    let close = matching_close(tokens, open);
    Some((tokens[open].start, tokens[close].end))
}

fn on_variable(tokens: &[Token], open: usize) -> bool {
    open >= 3
        && matches!(tokens[open - 1].kind, Kind::Ident(_))
        && tokens[open - 2].kind == Kind::ObjectOp
        && matches!(tokens[open - 3].kind, Kind::Var(_))
}

fn strings_in(tokens: &[Token], open: usize, close: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut arg = 0usize;
    for t in &tokens[open + 1..close] {
        match &t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => depth = depth.saturating_sub(1),
            Kind::Comma if depth == 0 => arg += 1,
            Kind::Str { value, .. } if depth <= 1 => {
                if out.len() <= arg {
                    out.resize(arg + 1, String::new());
                }
                if out[arg].is_empty() {
                    out[arg].clone_from(value);
                }
            }
            _ => {}
        }
    }
    out
}

fn chain(tokens: &[Token], open: usize) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut close = matching_close(tokens, open);
    while tokens.get(close + 1).map(|t| &t.kind) == Some(&Kind::ObjectOp)
        && let Some(Kind::Ident(name)) = tokens.get(close + 2).map(|t| &t.kind)
        && tokens.get(close + 3).map(|t| &t.kind) == Some(&Kind::Open('('))
    {
        let next_open = close + 3;
        let next_close = matching_close(tokens, next_open);
        out.push((name.clone(), strings_in(tokens, next_open, next_close)));
        close = next_close;
    }
    out
}

fn constrained_table(column: &str, args: &[String]) -> String {
    match args.first().filter(|t| !t.is_empty()) {
        Some(t) => t.clone(),
        None => crate::index::plural(column.strip_suffix("_id").unwrap_or(column)),
    }
}

fn index_kind(method: &str) -> Option<&'static str> {
    Some(match method.to_ascii_lowercase().as_str() {
        "primary" => "primary",
        "unique" => "unique",
        "index" => "index",
        "fulltext" => "fulltext",
        "spatialindex" => "spatial",
        _ => return None,
    })
}

struct Site<'a> {
    tokens: &'a [Token],
    open: usize,
    table: &'a str,
    start: usize,
    end: usize,
}

impl Site<'_> {
    fn index(&self, columns: Vec<String>, kind: &str) -> (usize, Op) {
        (
            self.start,
            Op::Index(Index {
                table: self.table.to_owned(),
                columns,
                kind: kind.to_owned(),
                start: self.start,
                end: self.end,
            }),
        )
    }

    fn modifiers(&self, ops: &mut Vec<(usize, Op)>, columns: &[String]) {
        let first = columns.first().cloned().unwrap_or_default();
        let mut references = None;
        let mut on = None;
        for (name, args) in chain(self.tokens, self.open) {
            match name.as_str() {
                "constrained" => {
                    references = Some(
                        args.get(1)
                            .filter(|c| !c.is_empty())
                            .cloned()
                            .unwrap_or_else(|| "id".into()),
                    );
                    on = Some(constrained_table(&first, &args));
                }
                "references" => references = args.first().cloned(),
                "on" => on = args.first().cloned(),
                other => {
                    if let Some(kind) = index_kind(other) {
                        ops.push(self.index(columns.to_vec(), kind));
                    }
                }
            }
        }
        let Some(on) = on.filter(|o| !o.is_empty()) else {
            return;
        };
        for column in columns {
            ops.push((
                self.start,
                Op::Foreign(Foreign {
                    table: self.table.to_owned(),
                    column: column.clone(),
                    on: on.clone(),
                    references: references.clone().unwrap_or_else(|| "id".into()),
                    start: self.start,
                    end: self.end,
                }),
            ));
        }
    }
}

fn morph_columns(name: &str) -> Vec<String> {
    vec![format!("{name}_type"), format!("{name}_id")]
}

fn collect_calls(tokens: &[Token], outside: &dyn Fn(usize) -> bool) -> Vec<Call> {
    let mut calls: Vec<Call> = Vec::new();
    for a in string_args(tokens) {
        if !outside(a.start) {
            continue;
        }
        let c = &a.callee;
        let schema = c.kind == CallKind::Static && c.class.as_deref() == Some("schema");
        let on_table =
            c.kind == CallKind::Method && a.head.as_deref().is_some_and(|h| h.starts_with('$'));
        if !schema && !on_table {
            continue;
        }
        match calls
            .iter_mut()
            .rev()
            .find(|k| k.open == a.call_start && k.name == c.name && k.schema == schema)
        {
            Some(call) => call.args.push((a.arg, a.value.clone(), a.start, a.end)),
            None => calls.push(Call {
                name: c.name.clone(),
                schema,
                open: a.call_start,
                args: vec![(a.arg, a.value.clone(), a.start, a.end)],
            }),
        }
    }
    calls
}

fn arg(call: &Call, n: usize) -> Option<&str> {
    call.args
        .iter()
        .find(|(k, ..)| *k == n)
        .map(|(_, v, ..)| v.as_str())
}

fn schema_op(call: &Call) -> Option<Op> {
    let (_, first, start, end) = call.args.first()?;
    Some(match call.name.as_str() {
        "create" | "table" if call.args[0].0 == 0 => Op::Table(Table {
            name: first.clone(),
            start: *start,
            end: *end,
            created: call.name == "create",
        }),
        "drop" | "dropIfExists" if call.args[0].0 == 0 => Op::DropTable(first.clone()),
        "rename" => Op::RenameTable {
            from: arg(call, 0)?.to_owned(),
            to: arg(call, 1)?.to_owned(),
        },
        _ => return None,
    })
}

fn table_call(call: &Call, site: &Site, ops: &mut Vec<(usize, Op)>) {
    let table = site.table;
    let first_args: Vec<String> = call
        .args
        .iter()
        .filter(|(k, ..)| *k == 0)
        .map(|(_, v, ..)| v.clone())
        .collect();
    let Some(first) = first_args.first().cloned() else {
        return;
    };
    let drop = |name: String| {
        (
            site.start,
            Op::DropColumn {
                table: table.to_owned(),
                name,
            },
        )
    };
    if let Some(kind) = COLUMN_TYPES
        .iter()
        .find(|t| t.eq_ignore_ascii_case(&call.name))
    {
        if call.args[0].0 != 0 || call.args.iter().filter(|(k, ..)| *k == 0).count() > 1 {
            return;
        }
        let names = if MORPHS.contains(kind) {
            vec![format!("{first}_id"), format!("{first}_type")]
        } else {
            vec![first.clone()]
        };
        for name in names {
            ops.push((
                site.start,
                Op::Column(Column {
                    table: table.to_owned(),
                    name,
                    kind: (*kind).to_owned(),
                    start: site.start,
                    end: site.end,
                }),
            ));
        }
        if MORPHS.contains(kind) {
            ops.push(site.index(morph_columns(&first), "index"));
        } else {
            if INCREMENTS.contains(kind) {
                ops.push(site.index(vec![first.clone()], "primary"));
            }
            site.modifiers(ops, &[first]);
        }
        return;
    }
    match call.name.to_ascii_lowercase().as_str() {
        "dropcolumn" | "dropcolumns" | "dropconstrainedforeignid" => {
            ops.extend(call.args.iter().map(|(_, name, ..)| drop(name.clone())));
        }
        "dropmorphs" => ops.extend(morph_columns(&first).into_iter().map(drop)),
        "renamecolumn" => {
            if let Some(to) = arg(call, 1) {
                ops.push((
                    site.start,
                    Op::RenameColumn {
                        table: table.to_owned(),
                        from: first,
                        to: to.to_owned(),
                    },
                ));
            }
        }
        "foreign" => site.modifiers(ops, &first_args),
        name => {
            if let Some(kind) = index_kind(name) {
                ops.push(site.index(first_args, kind));
            }
        }
    }
}

fn implied(name: &str) -> Option<(&'static [&'static str], bool)> {
    Some(match name {
        "id" => (&["id"], false),
        "timestamps" | "timestampsTz" | "nullableTimestamps" => {
            (&["created_at", "updated_at"], false)
        }
        "softDeletes" | "softDeletesTz" => (&["deleted_at"], false),
        "rememberToken" => (&["remember_token"], false),
        "dropTimestamps" | "dropTimestampsTz" => (&["created_at", "updated_at"], true),
        "dropSoftDeletes" | "dropSoftDeletesTz" => (&["deleted_at"], true),
        "dropRememberToken" => (&["remember_token"], true),
        _ => return None,
    })
}

pub fn parse(text: &str) -> Vec<Op> {
    let tokens = tokenize(text, 0, text.len());
    let skip = down_body(&tokens);
    let outside = |at: usize| !skip.is_some_and(|(s, e)| at >= s && at < e);
    let calls = collect_calls(&tokens, &outside);
    let mut ops: Vec<(usize, Op)> = Vec::new();
    for call in calls.iter().filter(|c| c.schema) {
        if let Some(op) = schema_op(call) {
            ops.push((call.args[0].2, op));
        }
    }
    let mut table_at: Vec<(usize, String)> = ops
        .iter()
        .filter_map(|(at, op)| match op {
            Op::Table(t) => Some((*at, t.name.clone())),
            _ => None,
        })
        .collect();
    table_at.sort();
    let table_before = |at: usize| {
        table_at
            .iter()
            .rev()
            .find(|(s, _)| *s < at)
            .map(|(_, t)| t.clone())
    };
    for call in calls.iter().filter(|c| !c.schema) {
        let Some(open) = tokens.iter().position(|t| t.start == call.open) else {
            continue;
        };
        let (_, _, start, end) = call.args[0];
        if !on_variable(&tokens, open) {
            continue;
        }
        let Some(table) = table_before(start) else {
            continue;
        };
        let site = Site {
            tokens: &tokens,
            open,
            table: &table,
            start,
            end,
        };
        table_call(call, &site, &mut ops);
    }
    for (i, t) in tokens.iter().enumerate() {
        let Kind::Ident(name) = &t.kind else {
            continue;
        };
        let no_args = tokens.get(i + 1).map(|n| &n.kind) == Some(&Kind::Open('('))
            && tokens.get(i + 2).map(|n| &n.kind) == Some(&Kind::Close(')'));
        if !no_args || !outside(t.start) || !on_variable(&tokens, i + 1) {
            continue;
        }
        let Some((columns, dropped)) = implied(name) else {
            continue;
        };
        let Some(table) = table_before(t.start) else {
            continue;
        };
        for col in columns {
            let op = if dropped {
                Op::DropColumn {
                    table: table.clone(),
                    name: (*col).to_owned(),
                }
            } else {
                Op::Column(Column {
                    table: table.clone(),
                    name: (*col).to_owned(),
                    kind: name.clone(),
                    start: t.start,
                    end: t.end,
                })
            };
            ops.push((t.start, op));
        }
        if name == "id" {
            let site = Site {
                tokens: &tokens,
                open: i + 1,
                table: &table,
                start: t.start,
                end: t.end,
            };
            ops.push(site.index(vec!["id".into()], "primary"));
            site.modifiers(&mut ops, &["id".to_owned()]);
        }
    }
    ops.sort_by_key(|(at, _)| *at);
    ops.into_iter().map(|(_, op)| op).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub path: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaColumn {
    pub name: String,
    pub kind: String,
    pub place: Place,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIndex {
    pub columns: Vec<String>,
    pub kind: String,
    pub place: Place,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaForeign {
    pub column: String,
    pub on: String,
    pub references: String,
    pub place: Place,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaTable {
    pub name: String,
    pub created: Option<Place>,
    pub columns: Vec<SchemaColumn>,
    pub indexes: Vec<SchemaIndex>,
    pub foreigns: Vec<SchemaForeign>,
}

impl SchemaTable {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            created: None,
            columns: Vec::new(),
            indexes: Vec::new(),
            foreigns: Vec::new(),
        }
    }

    pub fn leads_an_index(&self, column: &str) -> bool {
        self.indexes
            .iter()
            .any(|i| i.columns.first().is_some_and(|c| c == column))
            || self.foreigns.iter().any(|f| f.column == column)
    }

    fn drop_column(&mut self, name: &str) {
        self.columns.retain(|c| c.name != name);
        self.indexes
            .retain(|i| !i.columns.iter().any(|c| c == name));
        self.foreigns.retain(|f| f.column != name);
    }

    fn rename_column(&mut self, from: &str, to: &str) {
        for c in self.columns.iter_mut().filter(|c| c.name == from) {
            to.clone_into(&mut c.name);
        }
        for i in &mut self.indexes {
            for c in i.columns.iter_mut().filter(|c| *c == from) {
                to.clone_into(c);
            }
        }
        for f in self.foreigns.iter_mut().filter(|f| f.column == from) {
            to.clone_into(&mut f.column);
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Schema {
    pub tables: Vec<SchemaTable>,
}

impl Schema {
    pub fn table(&self, name: &str) -> Option<&SchemaTable> {
        self.tables.iter().find(|t| t.name == name)
    }

    fn table_mut(&mut self, name: &str) -> &mut SchemaTable {
        let at = match self.tables.iter().position(|t| t.name == name) {
            Some(at) => at,
            None => {
                self.tables.push(SchemaTable::new(name));
                self.tables.len() - 1
            }
        };
        &mut self.tables[at]
    }

    fn rename_table(&mut self, from: &str, to: &str) {
        if self.tables.iter().any(|t| t.name == to) {
            return;
        }
        if let Some(t) = self.tables.iter_mut().find(|t| t.name == from) {
            to.clone_into(&mut t.name);
        }
        for t in &mut self.tables {
            for f in t.foreigns.iter_mut().filter(|f| f.on == from) {
                to.clone_into(&mut f.on);
            }
        }
    }

    pub fn apply(&mut self, path: &str, text: &str) {
        let lines = Lines::new(text);
        let place = |start: usize, end: usize| Place {
            path: path.to_owned(),
            range: lines.range(text, start, end),
        };
        for op in parse(text) {
            match op {
                Op::Table(t) => {
                    let table = self.table_mut(&t.name);
                    if t.created && table.created.is_none() {
                        table.created = Some(place(t.start, t.end));
                    }
                }
                Op::Column(c) => {
                    let p = place(c.start, c.end);
                    let table = self.table_mut(&c.table);
                    if !table.columns.iter().any(|x| x.name == c.name) {
                        table.columns.push(SchemaColumn {
                            name: c.name,
                            kind: c.kind,
                            place: p,
                        });
                    }
                }
                Op::DropColumn { table, name } => self.table_mut(&table).drop_column(&name),
                Op::RenameColumn { table, from, to } => {
                    self.table_mut(&table).rename_column(&from, &to);
                }
                Op::DropTable(name) => self.tables.retain(|t| t.name != name),
                Op::RenameTable { from, to } => self.rename_table(&from, &to),
                Op::Index(i) => {
                    let p = place(i.start, i.end);
                    let table = self.table_mut(&i.table);
                    if !table
                        .indexes
                        .iter()
                        .any(|x| x.columns == i.columns && x.kind == i.kind)
                    {
                        table.indexes.push(SchemaIndex {
                            columns: i.columns,
                            kind: i.kind,
                            place: p,
                        });
                    }
                }
                Op::Foreign(f) => {
                    let p = place(f.start, f.end);
                    self.table_mut(&f.table).foreigns.push(SchemaForeign {
                        column: f.column,
                        on: f.on,
                        references: f.references,
                        place: p,
                    });
                }
            }
        }
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn migrations(project: &Project, root: &str) -> Vec<(String, String)> {
    let mut paths = find(project, root, "database/migrations/**/*.php", 10_000);
    paths.sort_by(|a, b| file_name(a).cmp(file_name(b)).then_with(|| a.cmp(b)));
    paths
        .into_iter()
        .filter_map(|p| {
            crate::project::read_listed(project, &p)
                .ok()
                .map(|t| (p, t))
        })
        .collect()
}

pub fn schema(project: &Project, root: &str) -> Schema {
    let mut schema = Schema::default();
    for (path, text) in migrations(project, root) {
        schema.apply(&path, &text);
    }
    schema
}

pub fn tables(project: &Project, root: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = schema(project, root)
        .tables
        .into_iter()
        .filter_map(|t| {
            let place = t.created?;
            Some(Entry::new(
                t.name,
                Some(file_name(&place.path).to_owned()),
                &place.path,
                place.range,
            ))
        })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn columns(project: &Project, root: &str) -> Vec<Entry> {
    schema(project, root)
        .tables
        .into_iter()
        .flat_map(|t| {
            let table = t.name;
            t.columns.into_iter().map(move |c| {
                Entry::new(
                    format!("{table}.{}", c.name),
                    Some(c.kind),
                    &c.place.path,
                    c.place.range,
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema_of(files: &[&str]) -> Schema {
        let mut schema = Schema::default();
        for (i, text) in files.iter().enumerate() {
            schema.apply(&format!("m{i}.php"), text);
        }
        schema
    }

    fn columns_of(schema: &Schema, table: &str) -> Vec<String> {
        schema
            .table(table)
            .map(|t| t.columns.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn reads_tables_and_columns_from_migrations() {
        let text = "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('posts', function (Blueprint $table) {\n            $table->id();\n            $table->foreignId('user_id')->constrained();\n            $table->string('title', 120)->nullable();\n            $table->morphs('commentable');\n            $table->timestamps();\n        });\n        Schema::table('users', function (Blueprint $table) {\n            $table->boolean('is_admin')->default(false);\n        });\n    }\n};\n";
        let ops = parse(text);
        let tables: Vec<_> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Table(t) => Some((t.name.as_str(), t.created)),
                _ => None,
            })
            .collect();
        assert_eq!(tables, [("posts", true), ("users", false)]);
        let cols: Vec<_> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Column(c) => Some(format!("{}.{}", c.table, c.name)),
                _ => None,
            })
            .collect();
        assert_eq!(
            cols,
            [
                "posts.id",
                "posts.user_id",
                "posts.title",
                "posts.commentable_id",
                "posts.commentable_type",
                "posts.created_at",
                "posts.updated_at",
                "users.is_admin",
            ]
        );
    }

    #[test]
    fn dropped_and_renamed_columns_and_tables_follow_the_migrations_in_order() {
        let create = "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->string('code')->unique();\n            $table->string('note');\n            $table->string('legacy');\n            $table->morphs('owner');\n            $table->timestamps();\n            $table->softDeletes();\n        });\n        Schema::create('drafts', function (Blueprint $table) {\n            $table->id();\n        });\n    }\n\n    public function down(): void\n    {\n        Schema::dropIfExists('orders');\n        Schema::table('orders', function (Blueprint $table) {\n            $table->dropColumn('code');\n        });\n    }\n};\n";
        let alter = "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::table('orders', function (Blueprint $table) {\n            $table->dropColumn(['legacy', 'note']);\n            $table->renameColumn('code', 'reference');\n            $table->dropMorphs('owner');\n            $table->dropSoftDeletes();\n            $table->string('status');\n        });\n        Schema::rename('orders', 'purchases');\n        Schema::drop('drafts');\n    }\n};\n";
        let schema = schema_of(&[create, alter]);
        assert_eq!(
            columns_of(&schema, "purchases"),
            ["id", "reference", "created_at", "updated_at", "status"]
        );
        assert!(schema.table("orders").is_none());
        assert!(schema.table("drafts").is_none());
        let purchases = schema.table("purchases").unwrap();
        assert!(purchases.leads_an_index("reference"));
        assert!(!purchases.leads_an_index("owner_type"));
    }

    #[test]
    fn reads_indexes_and_foreign_keys() {
        let text = "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('comments', function (Blueprint $table) {\n            $table->id();\n            $table->foreignId('post_id')->constrained()->cascadeOnDelete();\n            $table->foreignId('author_id')->constrained('users');\n            $table->unsignedBigInteger('category_id');\n            $table->foreign('category_id')->references('id')->on('categories');\n            $table->string('email')->index();\n            $table->string('slug')->unique('comments_slug_unique');\n            $table->string('status');\n            $table->index(['status', 'email']);\n            $table->unique('status', 'status_unique');\n        });\n    }\n};\n";
        let schema = schema_of(&[text]);
        let t = schema.table("comments").unwrap();
        let foreigns: Vec<_> = t
            .foreigns
            .iter()
            .map(|f| format!("{}->{}.{}", f.column, f.on, f.references))
            .collect();
        assert_eq!(
            foreigns,
            [
                "post_id->posts.id",
                "author_id->users.id",
                "category_id->categories.id"
            ]
        );
        let indexes: Vec<_> = t
            .indexes
            .iter()
            .map(|i| format!("{}:{}", i.kind, i.columns.join("+")))
            .collect();
        assert_eq!(
            indexes,
            [
                "primary:id",
                "index:email",
                "unique:slug",
                "index:status+email",
                "unique:status"
            ]
        );
        assert_eq!(
            columns_of(&schema, "comments"),
            [
                "id",
                "post_id",
                "author_id",
                "category_id",
                "email",
                "slug",
                "status"
            ]
        );
    }
}
