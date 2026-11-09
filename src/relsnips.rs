use crate::index::plural;
use crate::members::singular;
use openidea_plugin::Range;
use openidea_plugin::providers::{CompletionItem, CompletionKind};
use std::collections::BTreeMap;

const RELATIONS: &str = "Illuminate\\Database\\Eloquent\\Relations\\";

#[derive(Clone, Copy)]
struct Kind {
    prefix: &'static str,
    class: &'static str,
    call: &'static str,
}

const KINDS: [Kind; 11] = [
    Kind {
        prefix: "hasone",
        class: "HasOne",
        call: "hasOne",
    },
    Kind {
        prefix: "hasmany",
        class: "HasMany",
        call: "hasMany",
    },
    Kind {
        prefix: "belongsto",
        class: "BelongsTo",
        call: "belongsTo",
    },
    Kind {
        prefix: "belongstomany",
        class: "BelongsToMany",
        call: "belongsToMany",
    },
    Kind {
        prefix: "hasonethrough",
        class: "HasOneThrough",
        call: "hasOneThrough",
    },
    Kind {
        prefix: "hasmanythrough",
        class: "HasManyThrough",
        call: "hasManyThrough",
    },
    Kind {
        prefix: "morphto",
        class: "MorphTo",
        call: "morphTo",
    },
    Kind {
        prefix: "morphone",
        class: "MorphOne",
        call: "morphOne",
    },
    Kind {
        prefix: "morphmany",
        class: "MorphMany",
        call: "morphMany",
    },
    Kind {
        prefix: "morphtomany",
        class: "MorphToMany",
        call: "morphToMany",
    },
    Kind {
        prefix: "morphedbymany",
        class: "MorphToMany",
        call: "morphedByMany",
    },
];

pub struct Facts<'a> {
    pub model: &'a str,
    pub table: &'a str,
    pub tables: &'a BTreeMap<String, Vec<String>>,
    pub models: &'a [(String, String)],
    pub existing: &'a [String],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub kind: &'static str,
    pub method: String,
    pub related: Option<String>,
    pub argument: Option<String>,
}

pub fn camel(snake: &str) -> String {
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

fn model_of_table<'a>(facts: &'a Facts, table: &str) -> Option<&'a str> {
    facts
        .models
        .iter()
        .find(|(_, t)| t == table)
        .map(|(name, _)| name.as_str())
}

pub fn suggestions(facts: &Facts) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    let push = |out: &mut Vec<Suggestion>, s: Suggestion| {
        if !facts.existing.contains(&s.method)
            && !out.iter().any(|o| o.kind == s.kind && o.method == s.method)
        {
            out.push(s);
        }
    };
    let own: &[String] = facts.tables.get(facts.table).map_or(&[], Vec::as_slice);
    for column in own {
        let Some(base) = column.strip_suffix("_id") else {
            continue;
        };
        let last = base.rsplit('_').next().unwrap_or(base);
        let related =
            model_of_table(facts, &plural(base)).or_else(|| model_of_table(facts, &plural(last)));
        if let Some(related) = related {
            push(
                &mut out,
                Suggestion {
                    kind: "belongsto",
                    method: camel(base),
                    related: Some(related.to_owned()),
                    argument: None,
                },
            );
        }
        if own.iter().any(|c| c == &format!("{base}_type")) {
            push(
                &mut out,
                Suggestion {
                    kind: "morphto",
                    method: camel(base),
                    related: None,
                    argument: None,
                },
            );
        }
    }
    let key = format!("{}_id", singular(facts.table));
    for (table, columns) in facts.tables {
        if table == facts.table {
            continue;
        }
        if columns.contains(&key) {
            if let Some(related) = model_of_table(facts, table) {
                push(
                    &mut out,
                    Suggestion {
                        kind: "hasmany",
                        method: camel(table),
                        related: Some(related.to_owned()),
                        argument: None,
                    },
                );
                push(
                    &mut out,
                    Suggestion {
                        kind: "hasone",
                        method: camel(&singular(table)),
                        related: Some(related.to_owned()),
                        argument: None,
                    },
                );
            } else {
                for other in columns.iter().filter(|c| *c != &key) {
                    let Some(base) = other.strip_suffix("_id") else {
                        continue;
                    };
                    if let Some(related) = model_of_table(facts, &plural(base)) {
                        push(
                            &mut out,
                            Suggestion {
                                kind: "belongstomany",
                                method: camel(&plural(base)),
                                related: Some(related.to_owned()),
                                argument: None,
                            },
                        );
                    }
                }
            }
        }
        for column in columns {
            let Some(name) = column.strip_suffix("_type").filter(|n| n.ends_with("able")) else {
                continue;
            };
            if !columns.contains(&format!("{name}_id")) {
                continue;
            }
            if let Some(related) = model_of_table(facts, table) {
                push(
                    &mut out,
                    Suggestion {
                        kind: "morphmany",
                        method: camel(table),
                        related: Some(related.to_owned()),
                        argument: Some(name.to_owned()),
                    },
                );
                push(
                    &mut out,
                    Suggestion {
                        kind: "morphone",
                        method: camel(&singular(table)),
                        related: Some(related.to_owned()),
                        argument: Some(name.to_owned()),
                    },
                );
            }
        }
    }
    out
}

fn body(kind: Kind, s: &Suggestion) -> String {
    let call = match (&s.related, &s.argument) {
        (_, _) if kind.call == "morphTo" => "\\$this->morphTo()".to_owned(),
        (Some(related), Some(arg)) => format!("\\$this->{}({related}::class, '{arg}')", kind.call),
        (Some(related), None) => format!("\\$this->{}({related}::class)", kind.call),
        _ => format!("\\$this->{}()", kind.call),
    };
    format!(
        "public function {}(): {}\n{{\n\treturn {call};\n}}$0",
        s.method, kind.class
    )
}

fn choice_body(kind: Kind, models: &[&str]) -> String {
    let choice = format!("${{2|{}|}}", models.join(","));
    let call = match kind.call {
        "morphTo" => "\\$this->morphTo()".to_owned(),
        "hasOneThrough" | "hasManyThrough" => {
            format!(
                "\\$this->{}({choice}::class, ${{3|{}|}}::class)",
                kind.call,
                models.join(",")
            )
        }
        "morphOne" | "morphMany" | "morphToMany" | "morphedByMany" => {
            format!("\\$this->{}({choice}::class, '${{3:taggable}}')", kind.call)
        }
        _ => format!("\\$this->{}({choice}::class)", kind.call),
    };
    format!(
        "public function ${{1:related}}(): {}\n{{\n\treturn {call};\n}}$0",
        kind.class
    )
}

pub struct Context<'a> {
    pub typed: &'a str,
    pub replace: Range,
    pub text: &'a str,
    pub import: &'a dyn Fn(&str) -> Option<String>,
}

pub fn items(facts: &Facts, ctx: &Context) -> Vec<CompletionItem> {
    let typed = ctx.typed.to_ascii_lowercase();
    if typed.is_empty() {
        return Vec::new();
    }
    let found = suggestions(facts);
    let others: Vec<&str> = facts
        .models
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| *n != facts.model)
        .collect();
    let mut out = Vec::new();
    for kind in KINDS {
        if !kind.prefix.starts_with(&typed) {
            continue;
        }
        let mine: Vec<&Suggestion> = found.iter().filter(|s| s.kind == kind.prefix).collect();
        let relation = format!("{RELATIONS}{}", kind.class);
        if mine.is_empty() {
            if others.is_empty() && kind.call != "morphTo" {
                continue;
            }
            let imports = crate::snippets::import_edits(ctx.text, &[relation.as_str()]);
            out.push(CompletionItem {
                detail: Some(format!("{} with a model of this project", kind.call)),
                insert_text: choice_body(kind, &others),
                snippet: true,
                replace: Some(ctx.replace),
                sort_text: Some(format!("~{}~", kind.prefix)),
                filter_text: Some(kind.prefix.to_owned()),
                additional_edits: imports,
                ..CompletionItem::new(kind.prefix, CompletionKind::Snippet)
            });
            continue;
        }
        for s in mine {
            let mut wanted = vec![relation.clone()];
            if let Some(fqn) = s.related.as_deref().and_then(|r| (ctx.import)(r)) {
                wanted.push(fqn);
            }
            let wanted: Vec<&str> = wanted.iter().map(String::as_str).collect();
            let label = match &s.related {
                Some(related) => format!("{} {} → {related}", kind.prefix, s.method),
                None => format!("{} {}", kind.prefix, s.method),
            };
            out.push(CompletionItem {
                detail: Some(format!("{}() from the migrations", kind.call)),
                insert_text: body(kind, s),
                snippet: true,
                replace: Some(ctx.replace),
                sort_text: Some(format!("~{}{}", kind.prefix, s.method)),
                filter_text: Some(format!("{} {}", kind.prefix, s.method)),
                additional_edits: crate::snippets::import_edits(ctx.text, &wanted),
                ..CompletionItem::new(&label, CompletionKind::Snippet)
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> BTreeMap<String, Vec<String>> {
        let mut t = BTreeMap::new();
        let mut add = |name: &str, cols: &[&str]| {
            t.insert(
                name.to_owned(),
                cols.iter().map(|c| (*c).to_owned()).collect(),
            );
        };
        add("users", &["id", "name"]);
        add("addresses", &["id", "user_id", "street"]);
        add("orders", &["id", "user_id", "billing_address_id", "total"]);
        add("roles", &["id", "name"]);
        add("role_user", &["role_id", "user_id"]);
        add(
            "comments",
            &["id", "commentable_type", "commentable_id", "body"],
        );
        add("images", &["id", "imageable_type", "imageable_id"]);
        t
    }

    fn models() -> Vec<(String, String)> {
        [
            ("User", "users"),
            ("Address", "addresses"),
            ("Order", "orders"),
            ("Role", "roles"),
            ("Comment", "comments"),
        ]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
    }

    fn found(
        model: &str,
        table: &str,
        existing: &[String],
    ) -> Vec<(String, String, Option<String>)> {
        let tables = tables();
        let models = models();
        suggestions(&Facts {
            model,
            table,
            tables: &tables,
            models: &models,
            existing,
        })
        .into_iter()
        .map(|s| (s.kind.to_owned(), s.method, s.related))
        .collect()
    }

    #[test]
    fn predicts_relations_from_the_migrations() {
        let user = found("User", "users", &[]);
        assert!(user.contains(&("hasmany".into(), "addresses".into(), Some("Address".into()))));
        assert!(user.contains(&("hasone".into(), "address".into(), Some("Address".into()))));
        assert!(user.contains(&("hasmany".into(), "orders".into(), Some("Order".into()))));
        assert!(user.contains(&("belongstomany".into(), "roles".into(), Some("Role".into()))));
        let order = found("Order", "orders", &[]);
        assert!(order.contains(&("belongsto".into(), "user".into(), Some("User".into()))));
        assert!(order.contains(&(
            "belongsto".into(),
            "billingAddress".into(),
            Some("Address".into())
        )));
        assert!(order.contains(&(
            "morphmany".into(),
            "comments".into(),
            Some("Comment".into())
        )));
        let comment = found("Comment", "comments", &[]);
        assert!(comment.contains(&("morphto".into(), "commentable".into(), None)));
        let without = found("User", "users", &["orders".to_owned()]);
        assert!(!without.iter().any(|(_, m, _)| m == "orders"));
    }

    #[test]
    fn falls_back_to_a_choice_of_real_models() {
        let tables = tables();
        let models = models();
        let none = |_: &str| None;
        let ctx = Context {
            typed: "hasmanythrough",
            replace: Range::default(),
            text: "<?php\nnamespace App\\Models;\n\nclass Role extends Model\n{\n}\n",
            import: &none,
        };
        let role = items(
            &Facts {
                model: "Role",
                table: "roles",
                tables: &tables,
                models: &models,
                existing: &[],
            },
            &ctx,
        );
        assert_eq!(role.len(), 1);
        assert!(
            role[0]
                .insert_text
                .contains("${2|User,Address,Order,Comment|}"),
            "{}",
            role[0].insert_text
        );
        let ctx = Context {
            typed: "hasm",
            ..ctx
        };
        let user = items(
            &Facts {
                model: "User",
                table: "users",
                tables: &tables,
                models: &models,
                existing: &[],
            },
            &ctx,
        );
        let labels: Vec<&str> = user.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"hasmany addresses → Address"),
            "{labels:?}"
        );
        assert!(user[0].insert_text.contains("return \\$this->hasMany("));
    }
}
