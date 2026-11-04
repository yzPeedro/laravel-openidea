use super::Entry;
use super::views::name as view_name;
use crate::php::{CallKind, args_in};
use crate::project::{Lines, find, rel};
use openidea_plugin::{Project, Range};
use std::collections::BTreeSet;

pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn class_component(relative: &str, base: &str) -> Option<String> {
    let inner = relative.strip_prefix(base)?.strip_suffix(".php")?;
    Some(inner.split('/').map(kebab).collect::<Vec<_>>().join("."))
}

fn view_component(relative: &str, base: &str) -> Option<String> {
    let inner = relative.strip_prefix(base)?;
    let stem = inner
        .strip_suffix(".blade.php")
        .or_else(|| inner.strip_suffix(".php"))?;
    let name = stem.replace('/', ".");
    Some(
        name.strip_suffix(".index")
            .map(str::to_owned)
            .unwrap_or(name),
    )
}

fn collect(project: &Project, root: &str, sources: &[(&str, &str, bool)]) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (glob, base, class) in sources {
        for path in find(project, root, glob, 5000) {
            let relative = rel(root, &path).replace('\\', "/");
            let name = if *class {
                class_component(&relative, base)
            } else {
                view_component(&relative, base)
            };
            let Some(name) = name else { continue };
            if seen.insert(name.clone()) {
                out.push(Entry::new(name, Some(relative), &path, Range::default()));
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn components(project: &Project, root: &str) -> Vec<Entry> {
    collect(
        project,
        root,
        &[
            ("app/View/Components/**/*.php", "app/View/Components/", true),
            (
                "resources/views/components/**/*.blade.php",
                "resources/views/components/",
                false,
            ),
        ],
    )
}

pub fn livewire(project: &Project, root: &str) -> Vec<Entry> {
    collect(
        project,
        root,
        &[
            ("app/Livewire/**/*.php", "app/Livewire/", true),
            ("app/Http/Livewire/**/*.php", "app/Http/Livewire/", true),
            (
                "resources/views/livewire/**/*.blade.php",
                "resources/views/livewire/",
                false,
            ),
        ],
    )
}

fn directive_names(project: &Project, root: &str, directives: &[&str]) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for path in find(project, root, "resources/views/**/*.blade.php", 10_000) {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        if !directives.iter().any(|d| text.contains(&format!("@{d}("))) {
            continue;
        }
        let relative = rel(root, &path).replace('\\', "/");
        let view = view_name(&relative).unwrap_or(relative);
        let lines = Lines::new(&text);
        for a in args_in(&text, true) {
            if a.callee.kind == CallKind::Directive
                && a.arg == 0
                && directives.contains(&a.callee.name.as_str())
                && seen.insert(a.value.clone())
            {
                out.push(Entry::new(
                    a.value,
                    Some(format!("@{} in {view}", a.callee.name)),
                    &path,
                    lines.range(&text, a.start, a.end),
                ));
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn custom_directives(project: &Project, root: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for path in find(project, root, "app/**/*.php", 20_000) {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        if !text.contains("Blade::") {
            continue;
        }
        let lines = Lines::new(&text);
        for a in crate::php::string_args(&crate::php::tokenize(&text, 0, text.len())) {
            let c = &a.callee;
            if c.kind != CallKind::Static
                || c.class.as_deref() != Some("blade")
                || a.arg != 0
                || a.in_array
            {
                continue;
            }
            let names: Vec<(String, String)> = match c.name.as_str() {
                "directive" => vec![(a.value.clone(), "custom directive".into())],
                "if" => vec![
                    (a.value.clone(), format!("@{0}(…) … @end{0}", a.value)),
                    (format!("else{}", a.value), "custom condition".into()),
                    (format!("unless{}", a.value), "custom condition".into()),
                    (format!("end{}", a.value), format!("closes @{}", a.value)),
                ],
                _ => continue,
            };
            for (name, detail) in names {
                if seen.insert(name.clone()) {
                    out.push(Entry::new(
                        name,
                        Some(detail),
                        &path,
                        lines.range(&text, a.start, a.end),
                    ));
                }
            }
        }
    }
    out
}

pub fn sections(project: &Project, root: &str) -> Vec<Entry> {
    directive_names(project, root, &["yield", "section"])
}

pub fn stacks(project: &Project, root: &str) -> Vec<Entry> {
    directive_names(project, root, &["stack", "push", "prepend"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_components_livewire_and_layout_slots() {
        let host = openidea_plugin::mock::MockHost::install();
        host.add_file(
            "app/View/Components/Forms/TextInput.php",
            "<?php class TextInput {}",
        );
        host.add_file("resources/views/components/alert.blade.php", "<div></div>");
        host.add_file(
            "resources/views/components/card/index.blade.php",
            "<div></div>",
        );
        host.add_file("app/Livewire/UserTable.php", "<?php class UserTable {}");
        host.add_file(
            "resources/views/layouts/app.blade.php",
            "@yield('content')\n@stack('scripts')\n",
        );
        host.add_file(
            "resources/views/home.blade.php",
            "@section('title', 'Home')\n@push('scripts')<script></script>@endpush\n",
        );
        let project = openidea_plugin::host::projects().unwrap().remove(0);
        let keys = |v: Vec<Entry>| v.into_iter().map(|e| e.key).collect::<Vec<_>>();
        assert_eq!(
            keys(components(&project, "/project")),
            ["alert", "card", "forms.text-input"]
        );
        assert_eq!(keys(livewire(&project, "/project")), ["user-table"]);
        assert_eq!(keys(sections(&project, "/project")), ["content", "title"]);
        assert_eq!(keys(stacks(&project, "/project")), ["scripts"]);
    }
}
