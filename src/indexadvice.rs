use crate::index::database::Schema;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String),
    Quoted(String),
    Dot,
    Op,
    Open,
    Close,
    Other,
}

const STOP: [&str; 9] = [
    "group",
    "order",
    "limit",
    "having",
    "offset",
    "union",
    "for",
    "returning",
    "window",
];

const COMPARE: [&str; 6] = ["in", "like", "is", "between", "not", "ilike"];

const KEYWORDS: &[&str] = &[
    "where", "join", "inner", "left", "right", "cross", "outer", "on", "group", "order", "limit",
    "having", "offset", "union", "for", "set", "values", "using", "natural", "full", "and", "or",
    "not", "is", "null", "in", "like", "ilike", "between", "exists", "as", "select", "from",
    "true", "false", "case", "when", "then", "else", "end", "distinct", "asc", "desc",
];

fn tokens(sql: &str) -> Vec<Tok> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'`' | b'"' | b'[' => {
                let close = if c == b'[' { b']' } else { c };
                let end = b[i + 1..]
                    .iter()
                    .position(|&x| x == close)
                    .map_or(b.len(), |p| i + 1 + p);
                out.push(Tok::Quoted(sql[i + 1..end].to_ascii_lowercase()));
                i = end + 1;
            }
            b'\'' => {
                let mut j = i + 1;
                while j < b.len() {
                    if b[j] == b'\'' && b.get(j + 1) == Some(&b'\'') {
                        j += 2;
                    } else if b[j] == b'\'' {
                        break;
                    } else {
                        j += 1;
                    }
                }
                out.push(Tok::Other);
                i = j + 1;
            }
            b'.' => {
                out.push(Tok::Dot);
                i += 1;
            }
            b'(' => {
                out.push(Tok::Open);
                i += 1;
            }
            b')' => {
                out.push(Tok::Close);
                i += 1;
            }
            b'=' | b'<' | b'>' | b'!' => {
                while i < b.len() && matches!(b[i], b'=' | b'<' | b'>' | b'!') {
                    i += 1;
                }
                out.push(Tok::Op);
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$')
                {
                    i += 1;
                }
                out.push(Tok::Word(sql[start..i].to_ascii_lowercase()));
            }
            _ if c.is_ascii_whitespace() => i += 1,
            _ => {
                out.push(Tok::Other);
                i += 1;
            }
        }
    }
    out
}

fn name(t: Option<&Tok>) -> Option<&str> {
    match t? {
        Tok::Quoted(n) => Some(n),
        Tok::Word(n) if !KEYWORDS.contains(&n.as_str()) => Some(n),
        _ => None,
    }
}

fn is_word(t: Option<&Tok>, w: &str) -> bool {
    matches!(t, Some(Tok::Word(x)) if x == w)
}

fn qualified(toks: &[Tok], at: usize) -> Option<(Vec<String>, usize)> {
    let mut parts = vec![name(toks.get(at))?.to_owned()];
    let mut i = at + 1;
    while toks.get(i) == Some(&Tok::Dot) {
        parts.push(name(toks.get(i + 1))?.to_owned());
        i += 2;
    }
    Some((parts, i))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub table: String,
    pub filters: Vec<(String, String)>,
}

pub fn shape(sql: &str) -> Option<Shape> {
    let toks = tokens(sql);
    if !is_word(toks.first(), "select") {
        return None;
    }
    let mut depth = 0i32;
    let mut aliases: Vec<(String, String)> = Vec::new();
    let mut table = None;
    let mut in_where = false;
    let mut filters = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Open => depth += 1,
            Tok::Close => depth -= 1,
            Tok::Word(w) if depth == 0 => match w.as_str() {
                "from" | "join" => {
                    if let Some((parts, next)) = qualified(&toks, i + 1) {
                        let real = parts.last().cloned().unwrap_or_default();
                        let alias_at = if is_word(toks.get(next), "as") {
                            next + 1
                        } else {
                            next
                        };
                        let alias = name(toks.get(alias_at))
                            .filter(|_| toks.get(alias_at + 1) != Some(&Tok::Dot))
                            .map_or_else(|| real.clone(), str::to_owned);
                        if w == "from" && table.is_none() {
                            table = Some(real.clone());
                        }
                        aliases.push((alias, real.clone()));
                        aliases.push((real.clone(), real));
                        i = next;
                        continue;
                    }
                }
                "where" => in_where = true,
                other if STOP.contains(&other) => in_where = false,
                _ => {}
            },
            _ => {}
        }
        if in_where
            && depth == 0
            && let Some((parts, next)) = qualified(&toks, i)
        {
            let compares = match toks.get(next) {
                Some(Tok::Op) => true,
                Some(Tok::Word(w)) => COMPARE.contains(&w.as_str()),
                _ => false,
            };
            if compares {
                let column = parts.last().cloned().unwrap_or_default();
                let owner = if parts.len() >= 2 {
                    let q = &parts[parts.len() - 2];
                    aliases
                        .iter()
                        .find(|(a, _)| a == q)
                        .map_or_else(|| q.clone(), |(_, r)| r.clone())
                } else {
                    table.clone().unwrap_or_default()
                };
                filters.push((owner, column));
            }
            i = next;
            continue;
        }
        i += 1;
    }
    Some(Shape {
        table: table?,
        filters,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Advice {
    pub table: String,
    pub column: String,
    pub migration: Option<String>,
}

pub fn advise(schema: &Schema, sql: &str) -> Option<Advice> {
    let shape = shape(sql)?;
    let mut tables: Vec<&str> = shape.filters.iter().map(|(t, _)| t.as_str()).collect();
    tables.dedup();
    for owner in tables {
        let Some(t) = schema
            .tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(owner))
        else {
            continue;
        };
        let columns: Vec<&str> = shape
            .filters
            .iter()
            .filter(|(o, _)| o == owner)
            .map(|(_, c)| c.as_str())
            .filter(|c| t.columns.iter().any(|x| x.name.eq_ignore_ascii_case(c)))
            .collect();
        if columns.is_empty() || columns.iter().any(|c| t.leads_an_index(c)) {
            continue;
        }
        return Some(Advice {
            table: t.name.clone(),
            column: columns[0].to_owned(),
            migration: t.created.as_ref().map(|p| p.path.clone()),
        });
    }
    None
}

fn studly(text: &str) -> String {
    text.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_ascii_uppercase().to_string() + c.as_str()
            })
        })
        .collect()
}

pub fn migration(table: &str, column: &str, laravel_major: u32, now_secs: i64) -> (String, String) {
    let (y, m, d) = crate::logs::civil(now_secs.div_euclid(86_400));
    let secs = now_secs.rem_euclid(86_400);
    let stamp = format!(
        "{y:04}_{m:02}_{d:02}_{:02}{:02}{:02}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    );
    let slug = format!("add_{column}_index_to_{table}_table");
    let file = format!("database/migrations/{stamp}_{slug}.php");
    let body = |void: &str| {
        format!(
            "    public function up(){void}\n    {{\n        Schema::table('{table}', function (Blueprint $table) {{\n            $table->index('{column}');\n        }});\n    }}\n\n    public function down(){void}\n    {{\n        Schema::table('{table}', function (Blueprint $table) {{\n            $table->dropIndex(['{column}']);\n        }});\n    }}\n"
        )
    };
    let uses = "use Illuminate\\Database\\Migrations\\Migration;\nuse Illuminate\\Database\\Schema\\Blueprint;\nuse Illuminate\\Support\\Facades\\Schema;\n";
    let content = if laravel_major >= 9 {
        format!(
            "<?php\n\n{uses}\nreturn new class extends Migration\n{{\n{}}};\n",
            body(": void")
        )
    } else {
        format!(
            "<?php\n\n{uses}\nclass {} extends Migration\n{{\n{}}}\n",
            studly(&slug),
            body("")
        )
    };
    (file, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Schema {
        let mut schema = Schema::default();
        schema.apply(
            "/p/database/migrations/2024_01_01_000000_create_shop.php",
            "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('users', function (Blueprint $table) {\n            $table->id();\n            $table->string('email')->unique();\n            $table->string('status');\n        });\n        Schema::create('orders', function (Blueprint $table) {\n            $table->id();\n            $table->unsignedBigInteger('user_id');\n            $table->string('state');\n            $table->foreignId('shop_id')->constrained();\n        });\n    }\n};\n",
        );
        schema
    }

    #[test]
    fn reads_the_table_and_filtered_columns_of_a_query() {
        assert_eq!(
            shape(
                "select * from `orders` where `orders`.`user_id` = ? and `orders`.`user_id` is not null order by `id` desc limit 1"
            ),
            Some(Shape {
                table: "orders".into(),
                filters: vec![
                    ("orders".into(), "user_id".into()),
                    ("orders".into(), "user_id".into())
                ],
            })
        );
        assert_eq!(
            shape(
                "select \"o\".* from \"orders\" as \"o\" inner join \"users\" on \"users\".\"id\" = \"o\".\"user_id\" where \"users\".\"status\" in (?, ?) and state = 'x'"
            ),
            Some(Shape {
                table: "orders".into(),
                filters: vec![
                    ("users".into(), "status".into()),
                    ("orders".into(), "state".into())
                ],
            })
        );
        assert_eq!(shape("update orders set state = ?"), None);
    }

    #[test]
    fn suggests_an_index_only_when_no_filtered_column_leads_one() {
        let s = schema();
        let advice = advise(&s, "select * from `orders` where `orders`.`user_id` = ?").unwrap();
        assert_eq!(
            (advice.table.as_str(), advice.column.as_str()),
            ("orders", "user_id")
        );
        assert_eq!(
            advise(&s, "select * from `orders` where `orders`.`id` = ?"),
            None
        );
        assert_eq!(
            advise(
                &s,
                "select * from `orders` where `shop_id` = ? and `state` = ?"
            ),
            None
        );
        assert_eq!(advise(&s, "select * from `users` where `email` = ?"), None);
        assert_eq!(
            advise(&s, "select * from `users` where `status` = ?").map(|a| a.column),
            Some("status".into())
        );
        assert_eq!(advise(&s, "select * from `unknown` where `x` = ?"), None);
    }

    #[test]
    fn writes_a_migration_for_the_index() {
        let (file, content) = migration("orders", "user_id", 11, 1_731_234_567);
        assert_eq!(
            file,
            "database/migrations/2024_11_10_102927_add_user_id_index_to_orders_table.php"
        );
        assert!(content.contains("return new class extends Migration"));
        assert!(content.contains("$table->index('user_id');"));
        assert!(content.contains("$table->dropIndex(['user_id']);"));
        let (_, old) = migration("orders", "user_id", 8, 0);
        assert!(old.contains("class AddUserIdIndexToOrdersTable extends Migration"));
        assert!(old.contains("public function up()\n"));
    }
}
