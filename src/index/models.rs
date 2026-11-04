use super::Entry;
use super::classes::{Class, parse_class};
use crate::php::{Kind, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::Project;

const RELATIONS: [&str; 12] = [
    "hasOne",
    "hasMany",
    "belongsTo",
    "belongsToMany",
    "morphTo",
    "morphOne",
    "morphMany",
    "morphToMany",
    "morphedByMany",
    "hasOneThrough",
    "hasManyThrough",
    "hasOneOrMany",
];

const MODEL_BASES: [&str; 5] = ["Model", "Authenticatable", "Pivot", "MorphPivot", "User"];

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn plural(word: &str) -> String {
    let lower = word.to_lowercase();
    for (singular, plural) in [
        ("person", "people"),
        ("child", "children"),
        ("man", "men"),
        ("woman", "women"),
    ] {
        if let Some(stem) = lower.strip_suffix(singular) {
            return format!("{}{plural}", &word[..stem.len()]);
        }
    }
    if lower.ends_with('y')
        && !lower.ends_with("ay")
        && !lower.ends_with("ey")
        && !lower.ends_with("oy")
        && !lower.ends_with("uy")
    {
        format!("{}ies", &word[..word.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"]
        .iter()
        .any(|s| lower.ends_with(s))
    {
        format!("{word}es")
    } else {
        format!("{word}s")
    }
}

pub fn table_for(class: &Class) -> String {
    class.table.clone().unwrap_or_else(|| {
        let snake = snake(&class.name);
        match snake.rsplit_once('_') {
            Some((head, last)) => format!("{head}_{}", plural(last)),
            None => plural(&snake),
        }
    })
}

pub fn is_model(class: &Class) -> bool {
    class
        .extends
        .as_deref()
        .is_some_and(|e| MODEL_BASES.contains(&e) || e.ends_with("Model"))
}

fn models_in(project: &Project, root: &str) -> Vec<(String, String, Class)> {
    let mut paths = find(project, root, "app/Models/**/*.php", 5000);
    paths.extend(find(project, root, "app/**/Models/**/*.php", 5000));
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let text = project.read_file(&path).ok()?;
            let class = parse_class(&text)?;
            is_model(&class).then_some((path, text, class))
        })
        .collect()
}

pub fn models(project: &Project, root: &str) -> Vec<Entry> {
    models_in(project, root)
        .into_iter()
        .map(|(path, text, class)| {
            let lines = Lines::new(&text);
            Entry::new(
                class.name.clone(),
                Some(format!("table {}", table_for(&class))),
                &path,
                lines.range(&text, class.start, class.end),
            )
        })
        .collect()
}

pub fn relation_of(body: &str) -> Option<(String, Option<String>)> {
    let tokens = tokenize(body, 0, body.len());
    for (i, t) in tokens.iter().enumerate() {
        let Kind::Ident(name) = &t.kind else { continue };
        if !RELATIONS.contains(&name.as_str())
            || tokens.get(i + 1).map(|t| &t.kind) != Some(&Kind::Open('('))
        {
            continue;
        }
        let target = match (
            tokens.get(i + 2).map(|t| &t.kind),
            tokens.get(i + 3).map(|t| &t.kind),
        ) {
            (Some(Kind::Ident(c)), Some(Kind::DoubleColon)) => {
                Some(c.rsplit('\\').next().unwrap_or(c).to_owned())
            }
            (Some(Kind::Str { value, .. }), _) => {
                Some(value.rsplit('\\').next().unwrap_or(value).to_owned())
            }
            _ => None,
        };
        return Some((name.clone(), target));
    }
    None
}

pub fn relations(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for (path, text, class) in models_in(project, root) {
        let lines = Lines::new(&text);
        for m in class
            .methods
            .iter()
            .filter(|m| m.public && m.body.1 > m.body.0)
        {
            let Some((kind, target)) = relation_of(&text[m.body.0..m.body.1]) else {
                continue;
            };
            let detail = match target {
                Some(t) => format!("{kind}({t})"),
                None => kind,
            };
            out.push(Entry::new(
                format!("{}.{}", class.name, m.name),
                Some(detail),
                &path,
                lines.range(&text, m.start, m.end),
            ));
        }
    }
    out
}

pub fn scopes(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for (path, text, class) in models_in(project, root) {
        let lines = Lines::new(&text);
        for m in &class.methods {
            let Some(rest) = m
                .name
                .strip_prefix("scope")
                .filter(|r| r.chars().next().is_some_and(char::is_uppercase))
            else {
                let attributed = text[..m.start].rfind("#[Scope]").is_some_and(|a| {
                    !text[a..m.start].contains('}') && !text[a..m.start].contains(';')
                });
                if attributed {
                    out.push(Entry::new(
                        format!("{}.{}", class.name, m.name),
                        Some("scope".into()),
                        &path,
                        lines.range(&text, m.start, m.end),
                    ));
                }
                continue;
            };
            let mut name = rest.to_owned();
            name.replace_range(..1, &rest[..1].to_lowercase());
            out.push(Entry::new(
                format!("{}.{name}", class.name),
                Some("scope".into()),
                &path,
                lines.range(&text, m.start, m.end),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_tables_and_relations() {
        let class = parse_class("<?php class UserProfile extends Model {}").unwrap();
        assert_eq!(table_for(&class), "user_profiles");
        let class = parse_class("<?php class Category extends Model {}").unwrap();
        assert_eq!(table_for(&class), "categories");
        let class = parse_class("<?php class Address extends Model {}").unwrap();
        assert_eq!(table_for(&class), "addresses");
        assert_eq!(
            relation_of(" return $this->hasMany(\\App\\Models\\Post::class)->latest(); "),
            Some(("hasMany".into(), Some("Post".into())))
        );
        assert_eq!(
            relation_of(" return $this->morphTo(); "),
            Some(("morphTo".into(), None))
        );
        assert_eq!(relation_of(" return 1; "), None);
    }

    #[test]
    fn indexes_relations_and_scopes() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file(
            "app/Models/User.php",
            "<?php\nclass User extends Authenticatable\n{\n    public function posts(): HasMany\n    {\n        return $this->hasMany(Post::class);\n    }\n\n    public function scopeActive($q) { return $q; }\n\n    #[Scope]\n    protected function verified($q) {}\n}\n",
        );
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let rel: Vec<_> = relations(&project, "/project")
            .into_iter()
            .map(|e| (e.key, e.detail))
            .collect();
        assert_eq!(
            rel,
            [("User.posts".to_owned(), Some("hasMany(Post)".to_owned()))]
        );
        let sc: Vec<_> = scopes(&project, "/project")
            .into_iter()
            .map(|e| e.key)
            .collect();
        assert_eq!(sc, ["User.active", "User.verified"]);
        let m: Vec<_> = models(&project, "/project")
            .into_iter()
            .map(|e| (e.key, e.detail))
            .collect();
        assert_eq!(m, [("User".to_owned(), Some("table users".to_owned()))]);
    }
}
