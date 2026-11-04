use super::Entry;
use crate::php::{CallKind, Kind, StringArg, string_args, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::{Project, Range};
use std::collections::BTreeSet;

pub const ARTISAN: &[(&str, &str)] = &[
    ("about", "application overview"),
    ("cache:clear", "flush the application cache"),
    ("config:cache", "cache the configuration"),
    ("config:clear", "remove the configuration cache"),
    ("db:seed", "seed the database"),
    ("down", "maintenance mode"),
    ("event:cache", "cache events and listeners"),
    ("horizon", "start Horizon"),
    ("horizon:terminate", "terminate Horizon"),
    ("key:generate", "set the application key"),
    ("migrate", "run migrations"),
    ("migrate:fresh", "drop all tables and migrate"),
    ("migrate:rollback", "roll back the last batch"),
    ("model:prune", "prune models"),
    ("optimize", "cache framework bootstrap files"),
    ("optimize:clear", "remove cached bootstrap files"),
    ("queue:flush", "flush failed jobs"),
    ("queue:restart", "restart queue workers"),
    ("queue:retry", "retry failed jobs"),
    ("queue:work", "process jobs"),
    ("reverb:start", "start Reverb"),
    ("route:cache", "cache routes"),
    ("route:clear", "remove the route cache"),
    ("schedule:run", "run due scheduled commands"),
    ("storage:link", "create storage symlinks"),
    ("telescope:prune", "prune Telescope entries"),
    ("up", "leave maintenance mode"),
    ("view:cache", "compile Blade templates"),
    ("view:clear", "clear compiled views"),
];

pub const SOCIALITE: &[&str] = &[
    "bitbucket",
    "facebook",
    "github",
    "gitlab",
    "google",
    "linkedin",
    "linkedin-openid",
    "slack",
    "slack-openid",
    "twitch",
    "twitter",
    "twitter-oauth-2",
    "x",
];

fn scan<F>(project: &Project, root: &str, globs: &[&str], needle: &str, mut keep: F) -> Vec<Entry>
where
    F: FnMut(&StringArg) -> bool,
{
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for glob in globs {
        for path in find(project, root, glob, 20_000) {
            let Ok(text) = project.read_file(&path) else {
                continue;
            };
            if !text.contains(needle) {
                continue;
            }
            let lines = Lines::new(&text);
            let file = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned();
            for a in string_args(&tokenize(&text, 0, text.len())) {
                if keep(&a) && seen.insert(a.value.clone()) {
                    out.push(Entry::new(
                        a.value.clone(),
                        Some(file.clone()),
                        &path,
                        lines.range(&text, a.start, a.end),
                    ));
                }
            }
        }
    }
    out
}

fn is(a: &StringArg, kind: CallKind, class: Option<&str>, names: &[&str]) -> bool {
    a.callee.kind == kind
        && (class.is_none() || a.callee.class.as_deref() == class)
        && names.contains(&a.callee.name.as_str())
}

pub fn token_abilities(project: &Project, root: &str) -> Vec<Entry> {
    scan(
        project,
        root,
        &["app/**/*.php", "routes/**/*.php"],
        "createToken",
        |a| is(a, CallKind::Method, None, &["createtoken"]) && a.arg == 1 && a.in_array,
    )
}

pub fn channels(project: &Project, root: &str) -> Vec<Entry> {
    scan(project, root, &["routes/**/*.php"], "channel", |a| {
        is(a, CallKind::Static, Some("broadcast"), &["channel"]) && a.arg == 0
    })
}

pub fn queues(project: &Project, root: &str, config: &[Entry]) -> Vec<Entry> {
    let mut out = scan(project, root, &["app/**/*.php"], "onQueue", |a| {
        is(a, CallKind::Method, None, &["onqueue"]) && a.arg == 0
    });
    let mut seen: BTreeSet<String> = out.iter().map(|e| e.key.clone()).collect();
    for e in config
        .iter()
        .filter(|e| e.key.starts_with("queue.connections.") && e.key.ends_with(".queue"))
    {
        let Some(detail) = e.detail.as_deref() else {
            continue;
        };
        let Some(name) = detail.trim_end_matches(['\'', ')']).rsplit('\'').next() else {
            continue;
        };
        if !name.is_empty() && !name.contains(['(', ' ']) && seen.insert(name.to_owned()) {
            out.push(Entry {
                key: name.to_owned(),
                detail: Some(e.key.clone()),
                location: e.location.clone(),
            });
        }
    }
    out
}

pub fn commands(project: &Project, root: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut paths = find(project, root, "app/**/Commands/**/*.php", 5000);
    paths.extend(find(project, root, "routes/console.php", 1));
    for path in paths {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        let lines = Lines::new(&text);
        let tokens = tokenize(&text, 0, text.len());
        let mut found: Vec<(String, usize, usize)> = Vec::new();
        for (i, t) in tokens.iter().enumerate() {
            let Kind::Str { value, .. } = &t.kind else {
                continue;
            };
            let assigned = i >= 2
                && tokens[i - 1].kind == Kind::Assign
                && matches!(&tokens[i - 2].kind, Kind::Var(v) if v == "signature" || v == "name");
            let attribute = i >= 2
                && tokens[i - 1].kind == Kind::Colon
                && matches!(&tokens[i - 2].kind, Kind::Ident(n) if n == "name")
                && text[..t.start]
                    .rfind("AsCommand(")
                    .is_some_and(|p| !text[p..t.start].contains(")]"));
            if assigned || attribute {
                found.push((value.clone(), t.start, t.end));
            }
        }
        for a in string_args(&tokens) {
            if is(&a, CallKind::Static, Some("artisan"), &["command"]) && a.arg == 0 {
                found.push((a.value.clone(), a.start, a.end));
            }
        }
        for (signature, start, end) in found {
            let name = signature
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            if !name.is_empty() && seen.insert(name.clone()) {
                out.push(Entry::new(
                    name,
                    Some(signature.trim().to_owned()),
                    &path,
                    lines.range(&text, start, end),
                ));
            }
        }
    }
    for (name, detail) in ARTISAN {
        if seen.insert((*name).to_owned()) {
            out.push(Entry::new(
                *name,
                Some((*detail).to_owned()),
                "",
                Range::default(),
            ));
        }
    }
    out
}

fn spatie(project: &Project, root: &str, class: &str) -> Vec<Entry> {
    scan(
        project,
        root,
        &[
            "database/seeders/**/*.php",
            "database/migrations/**/*.php",
            "app/**/*.php",
        ],
        "Permission\\",
        |a| {
            let c = &a.callee;
            c.kind == CallKind::Static
                && c.class.as_deref() == Some(class)
                && (a.key.as_deref() == Some("name")
                    && matches!(
                        c.name.as_str(),
                        "create" | "firstorcreate" | "updateorcreate"
                    )
                    || !a.in_array
                        && a.arg == 0
                        && matches!(c.name.as_str(), "findorcreate" | "create"))
        },
    )
}

pub fn roles(project: &Project, root: &str) -> Vec<Entry> {
    spatie(project, root, "role")
}

pub fn permissions(project: &Project, root: &str) -> Vec<Entry> {
    spatie(project, root, "permission")
}

pub fn socialite(config: &[Entry]) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for e in config {
        let Some(rest) = e.key.strip_prefix("services.") else {
            continue;
        };
        let Some(name) = rest.strip_suffix(".client_id") else {
            continue;
        };
        if seen.insert(name.to_owned()) {
            out.push(Entry {
                key: name.to_owned(),
                detail: Some("config/services.php".into()),
                location: e.location.clone(),
            });
        }
    }
    for name in SOCIALITE {
        if seen.insert((*name).to_owned()) {
            out.push(Entry::new(
                *name,
                Some("Socialite provider".into()),
                "",
                Range::default(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_package_names() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file("app/Console/Commands/SendDigest.php", "<?php\nclass SendDigest extends Command { protected $signature = 'digest:send {--force}'; }\n");
        host.add_file(
            "routes/console.php",
            "<?php\nArtisan::command('inspire', fn () => 1);\n",
        );
        host.add_file(
            "routes/channels.php",
            "<?php\nBroadcast::channel('chat.{id}', fn () => true);\n",
        );
        host.add_file(
            "app/Http/Controllers/TokenController.php",
            "<?php\n$user->createToken('mobile', ['posts:read', 'posts:write']);\n",
        );
        host.add_file("database/seeders/RoleSeeder.php", "<?php\nuse Spatie\\Permission\\Models\\Role;\nRole::create(['name' => 'admin']);\nPermission::findOrCreate('edit articles');\n");
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let keys = |v: Vec<Entry>| v.into_iter().map(|e| e.key).collect::<Vec<_>>();
        let cmds = keys(commands(&project, "/project"));
        assert_eq!(&cmds[..2], ["digest:send", "inspire"]);
        assert_eq!(keys(channels(&project, "/project")), ["chat.{id}"]);
        assert_eq!(
            keys(token_abilities(&project, "/project")),
            ["posts:read", "posts:write"]
        );
        assert_eq!(keys(roles(&project, "/project")), ["admin"]);
        assert_eq!(keys(permissions(&project, "/project")), ["edit articles"]);
    }
}
