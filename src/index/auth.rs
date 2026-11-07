use super::Entry;
use super::classes::parse_class;
use crate::php::{CallKind, string_args, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::Project;
use std::collections::BTreeSet;

const POLICY_SKIP: [&str; 3] = ["before", "after", "__construct"];

fn calls(project: &Project, root: &str, glob: &str, class: &str, name: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for path in find(project, root, glob, 5000) {
        let Ok(text) = crate::project::read_listed(project, &path) else {
            continue;
        };
        if !text
            .to_ascii_lowercase()
            .contains(&format!("{class}::{name}("))
        {
            continue;
        }
        let lines = Lines::new(&text);
        let file = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned();
        for a in string_args(&tokenize(&text, 0, text.len())) {
            if a.callee.kind == CallKind::Static
                && a.callee.class.as_deref() == Some(class)
                && a.callee.name == name
                && a.arg == 0
                && !a.in_array
            {
                out.push(Entry::new(
                    a.value,
                    Some(file.clone()),
                    &path,
                    lines.range(&text, a.start, a.end),
                ));
            }
        }
    }
    out
}

pub fn abilities(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = calls(project, root, "app/**/*.php", "gate", "define");
    let mut seen: BTreeSet<String> = out.iter().map(|e| e.key.clone()).collect();
    for path in find(project, root, "app/**/Policies/**/*.php", 2000) {
        let Ok(text) = crate::project::read_listed(project, &path) else {
            continue;
        };
        let Some(class) = parse_class(&text) else {
            continue;
        };
        let lines = Lines::new(&text);
        for m in class
            .methods
            .iter()
            .filter(|m| m.public && !POLICY_SKIP.contains(&m.name.as_str()))
        {
            if seen.insert(m.name.clone()) {
                out.push(Entry::new(
                    m.name.clone(),
                    Some(class.name.clone()),
                    &path,
                    lines.range(&text, m.start, m.end),
                ));
            }
        }
    }
    out
}

pub fn rate_limiters(project: &Project, root: &str) -> Vec<Entry> {
    calls(project, root, "app/**/*.php", "ratelimiter", "for")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_gates_policies_and_limiters() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file(
            "app/Providers/AppServiceProvider.php",
            "<?php\nclass AppServiceProvider {\n  public function boot() {\n    Gate::define('admin-only', fn () => true);\n    RateLimiter::for('uploads', fn () => Limit::perMinute(5));\n  }\n}\n",
        );
        host.add_file(
            "app/Policies/PostPolicy.php",
            "<?php\nclass PostPolicy {\n  public function before($u) {}\n  public function update(User $u, Post $p): bool { return true; }\n  private function helper() {}\n}\n",
        );
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let a: Vec<_> = abilities(&project, "/project")
            .into_iter()
            .map(|e| (e.key, e.detail))
            .collect();
        assert_eq!(
            a,
            [
                (
                    "admin-only".to_owned(),
                    Some("AppServiceProvider.php".to_owned())
                ),
                ("update".to_owned(), Some("PostPolicy".to_owned()))
            ]
        );
        let r: Vec<_> = rate_limiters(&project, "/project")
            .into_iter()
            .map(|e| e.key)
            .collect();
        assert_eq!(r, ["uploads"]);
    }
}
