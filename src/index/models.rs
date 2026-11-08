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

fn parent_of(class: &Class) -> Option<&str> {
    let e = class.extends.as_deref()?;
    Some(e.rsplit('\\').next().unwrap_or(e))
}

fn models_in(project: &Project, root: &str) -> Vec<(String, String, Class)> {
    let mut paths = find(project, root, "app/Models/**/*.php", 5000);
    paths.extend(find(project, root, "app/**/Models/**/*.php", 5000));
    paths.sort();
    paths.dedup();
    let classes: Vec<(String, String, Class)> = paths
        .into_iter()
        .filter_map(|path| {
            let text = crate::project::read_listed(project, &path).ok()?;
            let class = parse_class(&text)?;
            Some((path, text, class))
        })
        .collect();
    let mut known: std::collections::BTreeSet<String> = classes
        .iter()
        .filter(|(_, _, c)| is_model(c))
        .map(|(_, _, c)| c.name.clone())
        .collect();
    loop {
        let more: Vec<String> = classes
            .iter()
            .filter(|(_, _, c)| {
                !known.contains(&c.name) && parent_of(c).is_some_and(|p| known.contains(p))
            })
            .map(|(_, _, c)| c.name.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        known.extend(more);
    }
    classes
        .into_iter()
        .filter(|(_, _, c)| known.contains(&c.name))
        .collect()
}

fn inherit(models: &[(String, String, Class)], mut own: Vec<Entry>) -> Vec<Entry> {
    let parents: std::collections::BTreeMap<&str, &str> = models
        .iter()
        .filter_map(|(_, _, c)| Some((c.name.as_str(), parent_of(c)?)))
        .collect();
    let mut inherited = Vec::new();
    for child in parents.keys() {
        let mut seen = std::collections::BTreeSet::new();
        let mut parent = parents.get(child).copied();
        while let Some(p) = parent
            && seen.insert(p)
        {
            let prefix = format!("{p}.");
            for e in own.iter().filter(|e| e.key.starts_with(&prefix)) {
                let key = format!("{child}.{}", &e.key[prefix.len()..]);
                let defined = own
                    .iter()
                    .chain(inherited.iter())
                    .any(|o: &Entry| o.key == key);
                if !defined {
                    inherited.push(Entry { key, ..e.clone() });
                }
            }
            parent = parents.get(p).copied();
        }
    }
    own.extend(inherited);
    own
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
    let models = models_in(project, root);
    let mut out = Vec::new();
    for (path, text, class) in &models {
        let lines = Lines::new(text);
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
                path,
                lines.range(text, m.start, m.end),
            ));
        }
    }
    inherit(&models, out)
}

const MANY: [&str; 3] = ["belongsToMany", "morphToMany", "morphedByMany"];

fn strings_in(tokens: &[crate::php::Token], open: usize) -> (Vec<String>, usize) {
    let mut depth = 0;
    let mut out = Vec::new();
    for (k, t) in tokens.iter().enumerate().skip(open) {
        match &t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => {
                depth -= 1;
                if depth == 0 {
                    return (out, k);
                }
            }
            Kind::Str { value, .. } => out.push(value.clone()),
            _ => {}
        }
    }
    (out, tokens.len())
}

pub fn pivot_of(model: &str, body: &str) -> Option<(String, Vec<String>)> {
    let tokens = tokenize(body, 0, body.len());
    let at = tokens
        .iter()
        .position(|t| matches!(&t.kind, Kind::Ident(n) if MANY.contains(&n.as_str())))?;
    let Kind::Ident(kind) = &tokens[at].kind else {
        return None;
    };
    if tokens.get(at + 1).map(|t| &t.kind) != Some(&Kind::Open('(')) {
        return None;
    }
    let related = match tokens.get(at + 2).map(|t| &t.kind) {
        Some(Kind::Ident(c)) => c.rsplit('\\').next().unwrap_or(c).to_owned(),
        Some(Kind::Str { value, .. }) => value.rsplit('\\').next().unwrap_or(value).to_owned(),
        _ => return None,
    };
    let (args, close) = strings_in(&tokens, at + 1);
    let mut keys = Vec::new();
    if kind == "belongsToMany" {
        let parent_key = format!("{}_id", snake(model));
        let related_key = format!("{}_id", snake(&related));
        keys.push(args.get(1).cloned().unwrap_or(parent_key));
        keys.push(args.get(2).cloned().unwrap_or(related_key));
    } else {
        let name = args.first().cloned().unwrap_or_default();
        if !name.is_empty() {
            keys.push(format!("{name}_id"));
            keys.push(format!("{name}_type"));
        }
        let other = if kind == "morphToMany" {
            &related
        } else {
            model
        };
        keys.push(
            args.get(2)
                .cloned()
                .unwrap_or(format!("{}_id", snake(other))),
        );
    }
    let mut accessor = "pivot".to_owned();
    let mut k = close;
    while k < tokens.len() {
        let call = match (&tokens[k].kind, tokens.get(k + 1).map(|t| &t.kind)) {
            (Kind::Ident(n), Some(Kind::Open('('))) => n.clone(),
            _ => {
                k += 1;
                continue;
            }
        };
        let (strings, end) = strings_in(&tokens, k + 1);
        match call.as_str() {
            "withPivot" => keys.extend(strings),
            "withTimestamps" => {
                keys.push(strings.first().cloned().unwrap_or("created_at".into()));
                keys.push(strings.get(1).cloned().unwrap_or("updated_at".into()));
            }
            "as" => accessor = strings.first().cloned().unwrap_or(accessor),
            _ => {}
        }
        k = end + 1;
    }
    let mut seen = std::collections::BTreeSet::new();
    keys.retain(|c| !c.is_empty() && seen.insert(c.clone()));
    Some((accessor, keys))
}

pub fn pivots(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for (path, text, class) in models_in(project, root) {
        let lines = Lines::new(&text);
        for m in class
            .methods
            .iter()
            .filter(|m| m.public && m.body.1 > m.body.0)
        {
            let Some((accessor, columns)) = pivot_of(&class.name, &text[m.body.0..m.body.1]) else {
                continue;
            };
            for column in columns {
                out.push(Entry::new(
                    format!("{accessor}.{column}"),
                    Some(format!("pivot of {}::{}()", class.name, m.name)),
                    &path,
                    lines.range(&text, m.start, m.end),
                ));
            }
        }
    }
    out
}

pub fn scopes(project: &Project, root: &str) -> Vec<Entry> {
    let models = models_in(project, root);
    let mut out = Vec::new();
    for (path, text, class) in &models {
        let lines = Lines::new(text);
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
                        path,
                        lines.range(text, m.start, m.end),
                    ));
                }
                continue;
            };
            let mut name = rest.to_owned();
            name.replace_range(..1, &rest[..1].to_lowercase());
            out.push(Entry::new(
                format!("{}.{name}", class.name),
                Some("scope".into()),
                path,
                lines.range(text, m.start, m.end),
            ));
        }
    }
    inherit(&models, out)
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
    fn reads_pivot_attributes() {
        let body = "return $this->belongsToMany(Role::class)->withPivot('expires_at', 'granted_by')->withTimestamps();";
        assert_eq!(
            pivot_of("User", body),
            Some((
                "pivot".to_owned(),
                [
                    "user_id",
                    "role_id",
                    "expires_at",
                    "granted_by",
                    "created_at",
                    "updated_at"
                ]
                .map(String::from)
                .to_vec()
            ))
        );
        let body = "return $this->belongsToMany(Podcast::class, 'subscriptions', 'member_id')->as('subscription')->withPivot(['active']);";
        assert_eq!(
            pivot_of("User", body),
            Some((
                "subscription".to_owned(),
                ["member_id", "podcast_id", "active"]
                    .map(String::from)
                    .to_vec()
            ))
        );
        let body = "return $this->morphToMany(Tag::class, 'taggable');";
        assert_eq!(
            pivot_of("Post", body).map(|(_, c)| c),
            Some(
                ["taggable_id", "taggable_type", "tag_id"]
                    .map(String::from)
                    .to_vec()
            )
        );
        assert_eq!(
            pivot_of("User", "return $this->hasMany(Post::class);"),
            None
        );
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
