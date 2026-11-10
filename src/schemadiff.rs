use crate::index::database::{Schema, SchemaTable};
use crate::project::rel;
use crate::sections::Row;
use serde::Deserialize;
use std::collections::BTreeSet;

const INTERNAL: [&str; 3] = ["migrations", "sqlite_sequence", "spatial_ref_sys"];

#[derive(Deserialize, Debug, Default, Clone)]
pub struct LiveIndex {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct LiveColumn {
    pub name: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct LiveTable {
    pub name: String,
    #[serde(default)]
    pub columns: Vec<LiveColumn>,
    #[serde(default)]
    pub indexes: Vec<LiveIndex>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Live {
    #[serde(default)]
    pub tables: Vec<LiveTable>,
    #[serde(default)]
    pub pending: Vec<String>,
}

pub const COLUMNS: [&str; 3] = ["Table", "Status", "Details"];

fn names<'a>(items: impl Iterator<Item = &'a String>) -> BTreeSet<String> {
    items.map(|n| n.to_ascii_lowercase()).collect()
}

fn listed(label: &str, items: &BTreeSet<String>) -> Option<String> {
    (!items.is_empty()).then(|| {
        format!(
            "{label}: {}",
            items.iter().cloned().collect::<Vec<_>>().join(", ")
        )
    })
}

fn index_keys<'a>(sets: impl Iterator<Item = &'a Vec<String>>) -> BTreeSet<String> {
    sets.filter(|c| !c.is_empty())
        .map(|c| c.join("+").to_ascii_lowercase())
        .collect()
}

fn table_row(root: &str, table: &SchemaTable, cells: Vec<String>) -> Row {
    match &table.created {
        Some(p) => Row::at(cells, &p.path, p.range.start.line + 1),
        None => match table.columns.first() {
            Some(c) => Row::at(cells, &c.place.path, c.place.range.start.line + 1),
            None => Row::at(cells, "", 0),
        },
    }
    .relative(root)
}

trait Relative {
    fn relative(self, root: &str) -> Self;
}

impl Relative for Row {
    fn relative(mut self, root: &str) -> Self {
        if let Some(t) = &mut self.open {
            t.path = rel(root, &t.path).to_owned();
        }
        self
    }
}

pub fn compare(root: &str, schema: &Schema, live: &Live) -> Vec<Row> {
    let mut rows = Vec::new();
    for name in &live.pending {
        rows.push(Row::at(
            vec![
                "—".into(),
                "Pending migration".into(),
                format!("{name} has not run on this database"),
            ],
            "",
            0,
        ));
    }
    let live_names: BTreeSet<String> = live
        .tables
        .iter()
        .map(|t| t.name.to_ascii_lowercase())
        .collect();
    for table in schema.tables.iter().filter(|t| t.created.is_some()) {
        let Some(found) = live
            .tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&table.name))
        else {
            rows.push(table_row(
                root,
                table,
                vec![
                    table.name.clone(),
                    "Only in migrations".into(),
                    "The migrations create this table but the database doesn't have it".into(),
                ],
            ));
            continue;
        };
        let ours = names(table.columns.iter().map(|c| &c.name));
        let theirs = names(found.columns.iter().map(|c| &c.name));
        let missing: BTreeSet<String> = ours.difference(&theirs).cloned().collect();
        let extra: BTreeSet<String> = theirs.difference(&ours).cloned().collect();
        let our_indexes = index_keys(
            table
                .indexes
                .iter()
                .filter(|i| i.kind != "primary")
                .map(|i| &i.columns),
        );
        let their_indexes = index_keys(
            found
                .indexes
                .iter()
                .filter(|i| !i.primary)
                .map(|i| &i.columns),
        );
        let missing_indexes: BTreeSet<String> =
            our_indexes.difference(&their_indexes).cloned().collect();
        let details: Vec<String> = [
            listed("Missing columns", &missing),
            listed("Columns only in the database", &extra),
            listed("Missing indexes", &missing_indexes),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !details.is_empty() {
            rows.push(table_row(
                root,
                table,
                vec![table.name.clone(), "Differs".into(), details.join(" · ")],
            ));
        }
    }
    let ours: BTreeSet<String> = schema
        .tables
        .iter()
        .filter(|t| t.created.is_some())
        .map(|t| t.name.to_ascii_lowercase())
        .collect();
    for name in &live_names {
        if !ours.contains(name) && !INTERNAL.contains(&name.as_str()) {
            let shown = live
                .tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(name))
                .map_or_else(|| name.clone(), |t| t.name.clone());
            rows.push(Row::at(
                vec![
                    shown,
                    "Only in the database".into(),
                    "No migration of the app creates it; a package or raw SQL may have".into(),
                ],
                "",
                0,
            ));
        }
    }
    rows
}

fn place_json(root: &str, place: Option<&crate::index::database::Place>) -> serde_json::Value {
    place.map_or(
        serde_json::Value::Null,
        |p| serde_json::json!({ "path": rel(root, &p.path), "line": p.range.start.line + 1 }),
    )
}

pub fn diagram(
    root: &str,
    schema: &Schema,
    table_of: &std::collections::BTreeMap<String, String>,
    relations: &[(String, String)],
) -> serde_json::Value {
    let created: Vec<&SchemaTable> = schema
        .tables
        .iter()
        .filter(|t| t.created.is_some())
        .collect();
    let known: BTreeSet<&str> = created.iter().map(|t| t.name.as_str()).collect();
    let mut tables = Vec::new();
    let mut links = Vec::new();
    let mut joined: BTreeSet<(String, String)> = BTreeSet::new();
    for t in &created {
        let columns: Vec<serde_json::Value> = t
            .columns
            .iter()
            .map(|c| {
                let key =
                    if t.indexes
                        .iter()
                        .any(|i| i.kind == "primary" && i.columns.first() == Some(&c.name))
                    {
                        "pk"
                    } else if t.foreigns.iter().any(|f| f.column == c.name) {
                        "fk"
                    } else if t.indexes.iter().any(|i| {
                        i.kind == "unique" && i.columns.len() == 1 && i.columns[0] == c.name
                    }) {
                        "uk"
                    } else {
                        ""
                    };
                serde_json::json!({ "name": c.name, "kind": c.kind, "key": key })
            })
            .collect();
        let models: Vec<&String> = table_of
            .iter()
            .filter(|(_, table)| *table == &t.name)
            .map(|(model, _)| model)
            .collect();
        tables.push(serde_json::json!({
            "name": t.name,
            "columns": columns,
            "models": models,
            "open": place_json(root, t.created.as_ref()),
        }));
        for f in &t.foreigns {
            if !known.contains(f.on.as_str()) {
                continue;
            }
            joined.insert((t.name.clone(), f.on.clone()));
            joined.insert((f.on.clone(), t.name.clone()));
            links.push(serde_json::json!({
                "from": t.name,
                "column": f.column,
                "to": f.on,
                "references": f.references,
                "kind": "foreign key",
            }));
        }
    }
    for (key, detail) in relations {
        let Some((model, method)) = key.split_once('.') else {
            continue;
        };
        let Some((kind, target)) = detail.split_once('(') else {
            continue;
        };
        let target = target.trim_end_matches(')');
        let (Some(from), Some(to)) = (table_of.get(model), table_of.get(target)) else {
            continue;
        };
        if from == to
            || !known.contains(from.as_str())
            || !known.contains(to.as_str())
            || !joined.insert((from.clone(), to.clone()))
        {
            continue;
        }
        joined.insert((to.clone(), from.clone()));
        links.push(serde_json::json!({
            "from": from,
            "column": "",
            "to": to,
            "references": "",
            "kind": format!("{model}::{method}() {kind}"),
        }));
    }
    serde_json::json!({ "type": "diagram", "tables": tables, "links": links })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_tables_with_their_keys_and_links() {
        let mut schema = Schema::default();
        schema.apply(
            "/p/database/migrations/2024_01_01_000000_create_shop.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('users', function (Blueprint $table) {\n            $table->id();\n            $table->string('email')->unique();\n        });\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->foreignId('user_id')->constrained();\n        });\n        Schema::create('tags', function (Blueprint $table) {\n            $table->id();\n            $table->unsignedBigInteger('order_id');\n        });\n    }\n};\n",
        );
        let table_of = std::collections::BTreeMap::from([
            ("User".to_owned(), "users".to_owned()),
            ("Order".to_owned(), "orders".to_owned()),
            ("Tag".to_owned(), "tags".to_owned()),
        ]);
        let relations = [
            ("Order.user".to_owned(), "belongsTo(User)".to_owned()),
            ("Order.tags".to_owned(), "hasMany(Tag)".to_owned()),
        ];
        let d = diagram("/p", &schema, &table_of, &relations);
        let keys: Vec<String> = d["tables"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|t| {
                let name = t["name"].as_str().unwrap().to_owned();
                t["columns"].as_array().unwrap().iter().map(move |c| {
                    format!(
                        "{name}.{}:{}",
                        c["name"].as_str().unwrap(),
                        c["key"].as_str().unwrap()
                    )
                })
            })
            .collect();
        assert_eq!(
            keys,
            [
                "users.id:pk",
                "users.email:uk",
                "orders.id:pk",
                "orders.user_id:fk",
                "tags.id:pk",
                "tags.order_id:"
            ]
        );
        let links: Vec<String> = d["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                format!(
                    "{}->{} {}",
                    l["from"].as_str().unwrap(),
                    l["to"].as_str().unwrap(),
                    l["kind"].as_str().unwrap()
                )
            })
            .collect();
        assert_eq!(
            links,
            [
                "orders->users foreign key",
                "orders->tags Order::tags() hasMany"
            ]
        );
        assert_eq!(d["tables"][1]["models"], serde_json::json!(["Order"]));
        assert_eq!(
            d["tables"][0]["open"]["path"],
            "database/migrations/2024_01_01_000000_create_shop.php"
        );
    }

    #[test]
    fn compares_the_migrations_with_the_live_database() {
        let mut schema = Schema::default();
        schema.apply(
            "/p/database/migrations/2024_01_01_000000_create_orders.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->string('code')->unique();\n            $table->string('status')->index();\n        });\n        Schema::create('invoices', function (Blueprint $table) {\n            $table->id();\n        });\n        Schema::create('tags', function (Blueprint $table) {\n            $table->id();\n        });\n    }\n};\n",
        );
        let live: Live = serde_json::from_value(serde_json::json!({
            "tables": [
                { "name": "orders", "columns": [{ "name": "id" }, { "name": "code" }, { "name": "legacy" }], "indexes": [
                    { "columns": ["id"], "primary": true },
                    { "columns": ["code"], "primary": false }
                ] },
                { "name": "tags", "columns": [{ "name": "id" }], "indexes": [] },
                { "name": "migrations", "columns": [] },
                { "name": "telescope_entries", "columns": [] }
            ],
            "pending": ["2024_02_01_000000_add_total"]
        }))
        .unwrap();
        let rows = compare("/p", &schema, &live);
        let cells: Vec<Vec<String>> = rows.iter().map(|r| r.cells.clone()).collect();
        assert_eq!(
            cells,
            [
                vec![
                    "—".to_owned(),
                    "Pending migration".into(),
                    "2024_02_01_000000_add_total has not run on this database".into()
                ],
                vec![
                    "orders".into(),
                    "Differs".into(),
                    "Missing columns: status · Columns only in the database: legacy · Missing indexes: status".into()
                ],
                vec![
                    "invoices".into(),
                    "Only in migrations".into(),
                    "The migrations create this table but the database doesn't have it".into()
                ],
                vec![
                    "telescope_entries".into(),
                    "Only in the database".into(),
                    "No migration of the app creates it; a package or raw SQL may have".into()
                ],
            ]
        );
        assert_eq!(
            rows[1].open.as_ref().map(|t| (t.path.as_str(), t.line)),
            Some(("database/migrations/2024_01_01_000000_create_orders.php", 5))
        );
    }
}
