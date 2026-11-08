use crate::resources::{Scan, Symbol, short};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    pub label: String,
    pub detail: String,
    pub path: String,
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Group {
    pub title: String,
    pub rows: Vec<Row>,
}

const KINDS: [(&str, &str); 20] = [
    ("app/Http/Controllers/", "Controllers"),
    ("app/Http/Requests/", "Form Requests"),
    ("app/Http/Resources/", "API Resources"),
    ("app/Http/Middleware/", "Middleware"),
    ("app/Policies/", "Policies"),
    ("app/Observers/", "Observers"),
    ("app/Jobs/", "Jobs"),
    ("app/Events/", "Events"),
    ("app/Listeners/", "Listeners"),
    ("app/Notifications/", "Notifications"),
    ("app/Mail/", "Mail"),
    ("app/Livewire/", "Livewire"),
    ("app/Http/Livewire/", "Livewire"),
    ("app/Filament/", "Filament"),
    ("app/Nova/", "Nova"),
    ("app/Console/", "Commands"),
    ("app/Models/", "Models"),
    ("database/factories/", "Factories"),
    ("database/seeders/", "Seeders"),
    ("routes/", "Routes"),
];

pub const ORDER: [&str; 24] = [
    "Model",
    "Migrations",
    "Relations",
    "Scopes",
    "Casts",
    "Factories",
    "Seeders",
    "Controllers",
    "Form Requests",
    "API Resources",
    "Policies",
    "Observers",
    "Middleware",
    "Jobs",
    "Events",
    "Listeners",
    "Notifications",
    "Mail",
    "Livewire",
    "Filament",
    "Nova",
    "Commands",
    "Routes",
    "Other classes",
];

pub fn mentions(text: &str, word: &str) -> bool {
    first_offset(text, word).is_some()
}

pub fn first_offset(text: &str, word: &str) -> Option<usize> {
    let boundary = |c: Option<char>| !c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let at = from + i;
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if boundary(before) && boundary(after) && before != Some('$') {
            return Some(at);
        }
        from = at + word.len();
    }
    None
}

pub fn line_of(text: &str, offset: usize) -> u32 {
    text[..offset.min(text.len())].matches('\n').count() as u32 + 1
}

fn kind_of(symbol: &Symbol) -> &'static str {
    let path = symbol.path.replace('\\', "/");
    if symbol
        .extends
        .iter()
        .any(|e| matches!(short(e), "JsonResource" | "ResourceCollection"))
    {
        return "API Resources";
    }
    KINDS
        .iter()
        .find(|(prefix, _)| path.starts_with(prefix))
        .map_or("Other classes", |(_, kind)| kind)
}

fn uses(symbol: &Symbol, model: &str) -> bool {
    symbol
        .methods
        .iter()
        .any(|m| mentions(&m.params, model) || mentions(&m.body, model))
        || symbol.attributes.iter().any(|a| mentions(&a.args, model))
        || symbol.attributes.iter().any(|a| short(&a.name) == model)
        || symbol.extends.iter().any(|e| short(e) == model)
}

pub fn from_scan(scan: &Scan, model: &str) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut add = |title: &str, row: Row| match groups.iter_mut().find(|g| g.title == title) {
        Some(g) => {
            if !g
                .rows
                .iter()
                .any(|r| r.path == row.path && r.label == row.label)
            {
                g.rows.push(row);
            }
        }
        None => groups.push(Group {
            title: title.to_owned(),
            rows: vec![row],
        }),
    };
    for symbol in &scan.symbols {
        if symbol.name == model || !uses(symbol, model) {
            continue;
        }
        let detail = if symbol.namespace.is_empty() {
            symbol.keyword.clone()
        } else {
            format!("{}\\{}", symbol.namespace, symbol.name)
        };
        add(
            kind_of(symbol),
            Row {
                label: symbol.name.clone(),
                detail,
                path: symbol.path.clone(),
                line: symbol.line,
            },
        );
    }
    for call in &scan.calls {
        let path = call.path.replace('\\', "/");
        if short(&call.class) != model || !path.starts_with("routes/") {
            continue;
        }
        add(
            "Routes",
            Row {
                label: format!("{}::{}()", short(&call.class), call.method),
                detail: format!("{path}:{}", call.line),
                path: call.path.clone(),
                line: call.line,
            },
        );
    }
    groups
}

pub fn migrations(files: &[(String, String)], table: &str) -> Vec<Row> {
    let quoted = [format!("'{table}'"), format!("\"{table}\"")];
    files
        .iter()
        .filter_map(|(path, text)| {
            let at = quoted
                .iter()
                .filter_map(|q| {
                    text.match_indices(q.as_str()).map(|(i, _)| i).find(|&i| {
                        let before = &text[text[..i].rfind('\n').map_or(0, |n| n + 1)..i];
                        before.contains("Schema::") || before.contains("constrained(")
                    })
                })
                .min()?;
            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            let line_text = text[text[..at].rfind('\n').map_or(0, |n| n + 1)..]
                .lines()
                .next()
                .unwrap_or_default()
                .trim();
            let detail = if line_text.contains("Schema::create") {
                "creates the table"
            } else if line_text.contains("constrained(") {
                "points a foreign key at it"
            } else {
                "changes the table"
            };
            Some(Row {
                label: name.trim_end_matches(".php").to_owned(),
                detail: detail.to_owned(),
                path: path.clone(),
                line: line_of(text, at),
            })
        })
        .collect()
}

pub fn lines_mentioning(path: &str, text: &str, model: &str) -> Vec<Row> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| mentions(line, model) && !line.trim_start().starts_with("use "))
        .map(|(i, line)| {
            let shown: String = line.trim().chars().take(120).collect();
            Row {
                label: shown,
                detail: path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned(),
                path: path.to_owned(),
                line: i as u32 + 1,
            }
        })
        .collect()
}

pub fn sort(groups: &mut [Group]) {
    groups.sort_by_key(|g| {
        ORDER
            .iter()
            .position(|o| *o == g.title)
            .unwrap_or(ORDER.len())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::scan_text;

    #[test]
    fn finds_whole_words_only() {
        assert!(mentions("public function show(Order $order)", "Order"));
        assert!(!mentions("OrderStatus::Paid", "Order"));
        assert!(!mentions("$order->total", "order"));
        assert_eq!(line_of("a\nb\nOrder", 4), 3);
    }

    #[test]
    fn groups_the_classes_and_routes_that_use_the_model() {
        let mut scan = scan_text(
            "app/Http/Controllers/OrderController.php",
            "<?php\nnamespace App\\Http\\Controllers;\n\nclass OrderController\n{\n    public function show(Order $order) { return $order; }\n}\n",
        );
        let resource = scan_text(
            "app/Http/Resources/OrderResource.php",
            "<?php\nnamespace App\\Http\\Resources;\n\n/** @mixin \\App\\Models\\Order */\nclass OrderResource extends JsonResource\n{\n    public function toArray($request): array { return ['total' => Order::count()]; }\n}\n",
        );
        let routes = scan_text(
            "routes/web.php",
            "<?php\nRoute::get('/orders', fn () => Order::all());\n",
        );
        let other = scan_text(
            "app/Jobs/SendInvoice.php",
            "<?php\nclass SendInvoice\n{\n    public function handle(OrderStatus $status) {}\n}\n",
        );
        for part in [resource, routes, other] {
            scan.symbols.extend(part.symbols);
            scan.calls.extend(part.calls);
        }
        let mut groups = from_scan(&scan, "Order");
        sort(&mut groups);
        let titles: Vec<&str> = groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Controllers", "API Resources"]);
        assert_eq!(groups[0].rows[0].label, "OrderController");
        let lines = lines_mentioning(
            "routes/web.php",
            "<?php\nuse App\\Models\\Order;\nRoute::get('/orders', fn () => Order::all());\n",
            "Order",
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line, 3);
    }

    #[test]
    fn finds_the_migrations_of_a_table() {
        let files = vec![
            (
                "database/migrations/2024_01_01_create_orders_table.php".to_owned(),
                "<?php\nSchema::create('orders', function (Blueprint $table) {\n    $table->id();\n});\n".to_owned(),
            ),
            (
                "database/migrations/2024_02_01_create_payments_table.php".to_owned(),
                "<?php\nSchema::create('payments', function (Blueprint $table) {\n    $table->foreignId('order_id')->constrained('orders');\n});\n".to_owned(),
            ),
            (
                "database/migrations/2024_03_01_create_users_table.php".to_owned(),
                "<?php\nSchema::create('users', function (Blueprint $table) {\n    $table->string('orders_note');\n});\n".to_owned(),
            ),
        ];
        let rows = migrations(&files, "orders");
        let found: Vec<(&str, &str, u32)> = rows
            .iter()
            .map(|r| (r.label.as_str(), r.detail.as_str(), r.line))
            .collect();
        assert_eq!(
            found,
            [
                ("2024_01_01_create_orders_table", "creates the table", 2),
                (
                    "2024_02_01_create_payments_table",
                    "points a foreign key at it",
                    3
                )
            ]
        );
    }
}
